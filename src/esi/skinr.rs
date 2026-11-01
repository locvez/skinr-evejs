//! Keeps the Paragon Hub's public SKINR listings, the designs they sell and
//! the names of their creators and sellers in Postgres.
//!
//! Listings come from `/paragon-hub/skinr`, a cursor-paginated route: the
//! first run walks forward from the beginning (`after=0`), and each later run
//! continues from the last `after` cursor, which returns everything listed or
//! changed since. A design (`/cosmetics/skinr/{id}`) never changes, so each is
//! fetched once. Names come from `/universe/names`.

use std::time::Duration;

use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::client::{Error, EsiClient};

/// How many listings to ask for per page (ESI allows 10 to 100).
const PAGE_SIZE: u32 = 100;
/// Pages walked per run; the rest waits for the next run, which keeps one run
/// well inside the route's 150 tokens per 15 minutes.
const MAX_PAGES_PER_RUN: usize = 40;
/// Designs fetched per run (their route allows 12000 tokens per 15 minutes).
const MAX_DESIGNS_PER_RUN: usize = 10_000;
/// Bounded parallel requests let an initial catalogue catch up promptly.
const DESIGN_FETCHES: usize = 8;
/// IDs per `/universe/names` request.
const NAMES_PER_REQUEST: usize = 500;
/// The pause between the end of one run and the start of the next.
pub const SYNC_INTERVAL: Duration = Duration::from_secs(10 * 60);
/// Where the cursor is kept in `esi_sync_state`.
const CURSOR_KEY: &str = "paragon_hub_skinr_after";
/// The cursor value that means "from the beginning".
const FIRST_CURSOR: &str = "0";

#[derive(Debug, Deserialize)]
pub struct ListingsPage {
    #[serde(default)]
    pub cursor: Option<Cursor>,
    #[serde(default)]
    pub listings: Vec<Listing>,
}

#[derive(Debug, Deserialize)]
pub struct Cursor {
    #[serde(default)]
    pub after: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Listing {
    pub id: String,
    pub skinr_id: String,
    pub seller_id: i64,
    pub state: String,
    #[serde(default)]
    pub price: serde_json::Value,
    pub quantity: i64,
    pub created: String,
    pub expires: String,
    pub last_modified: String,
}

impl Listing {
    /// The asking price in PLEX, when the listing is priced in PLEX.
    pub fn price_plex(&self) -> Option<i64> {
        self.price.get("plex")?.as_i64()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Design {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub line: Option<String>,
    pub creator_id: i64,
    pub ship_type_id: i32,
    #[serde(default)]
    pub tier: Option<Tier>,
    pub layout: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tier {
    pub level: i32,
}

#[derive(Debug, Deserialize)]
pub struct Name {
    pub id: i64,
    pub name: String,
}

/// One page of listings changed after `cursor`, and the cursor to continue from.
pub async fn fetch_listings(esi: &EsiClient, cursor: &str) -> Result<ListingsPage, Error> {
    esi.get(&format!(
        "/paragon-hub/skinr?after={cursor}&limit={PAGE_SIZE}"
    ))
    .await
}

pub async fn fetch_design(esi: &EsiClient, skinr_id: &str) -> Result<Design, Error> {
    esi.get(&format!("/cosmetics/skinr/{skinr_id}")).await
}

/// Names for `ids`. ESI rejects a whole batch when one ID is unknown, so a
/// failed batch is split in halves until the bad ID is alone and dropped.
pub async fn fetch_names(esi: &EsiClient, ids: &[i64]) -> Vec<Name> {
    let mut names = Vec::new();
    let mut batches: Vec<&[i64]> = ids.chunks(NAMES_PER_REQUEST).collect();
    while let Some(batch) = batches.pop() {
        match esi.post::<_, Vec<Name>>("/universe/names", &batch).await {
            Ok(found) => names.extend(found),
            Err(error) if batch.len() > 1 => {
                tracing::debug!(%error, size = batch.len(), "splitting a names batch");
                let (left, right) = batch.split_at(batch.len() / 2);
                batches.push(left);
                batches.push(right);
            }
            Err(error) => tracing::warn!(%error, id = batch[0], "no name for this ID"),
        }
    }
    names
}

/// What one run did.
#[derive(Debug, Default)]
pub struct SyncReport {
    pub listings: usize,
    pub designs: usize,
    pub names: usize,
}

/// One sync run: new and changed listings, their designs, then names.
pub async fn sync(
    pool: &PgPool,
    esi: &EsiClient,
) -> Result<SyncReport, Box<dyn std::error::Error>> {
    let mut report = SyncReport::default();

    let mut cursor: String = sqlx::query_scalar("select value from esi_sync_state where key = $1")
        .bind(CURSOR_KEY)
        .fetch_optional(pool)
        .await?
        .unwrap_or_else(|| FIRST_CURSOR.to_owned());
    let mut listing_error = None;
    for _ in 0..MAX_PAGES_PER_RUN {
        let page = match fetch_listings(esi, &cursor).await {
            Ok(page) => page,
            Err(error) => {
                // Already stored listings can still be made viewable during an outage.
                listing_error = Some(error);
                break;
            }
        };
        if page.listings.is_empty() {
            break;
        }
        for listing in &page.listings {
            store_listing(pool, listing).await?;
        }
        report.listings += page.listings.len();
        match page.cursor.and_then(|cursor| cursor.after) {
            Some(next) => cursor = next,
            None => break,
        }
        // Saved per page, so an interrupted run resumes where it stopped.
        sqlx::query(
            "insert into esi_sync_state (key, value) values ($1, $2)
             on conflict (key) do update set value = excluded.value",
        )
        .bind(CURSOR_KEY)
        .bind(&cursor)
        .execute(pool)
        .await?;
    }

    let missing: Vec<String> = sqlx::query_scalar(
        "select l.skinr_id from skinr_listings l
         left join skinr_designs d on d.id = l.skinr_id
         where d.id is null and l.state = 'listed' and l.expires > now()
         group by l.skinr_id order by max(l.last_modified) desc, l.skinr_id limit $1",
    )
    .bind(MAX_DESIGNS_PER_RUN as i64)
    .fetch_all(pool)
    .await?;
    let mut designs = stream::iter(missing.into_iter().map(|id| async move {
        let result = fetch_design(esi, &id).await;
        (id, result)
    }))
    .buffer_unordered(DESIGN_FETCHES);
    while let Some((skinr_id, result)) = designs.next().await {
        match result {
            Ok(design) => {
                store_design(pool, &design).await?;
                report.designs += 1;
            }
            Err(error) => tracing::warn!(%error, %skinr_id, "could not fetch a SKINR design"),
        }
    }

    let unnamed: Vec<i64> = sqlx::query_scalar(
        "select id from (
             select creator_id as id from skinr_designs
             union select seller_id from skinr_listings
         ) ids where id not in (select id from esi_characters)",
    )
    .fetch_all(pool)
    .await?;
    for name in fetch_names(esi, &unnamed).await {
        sqlx::query(
            "insert into esi_characters (id, name) values ($1, $2)
             on conflict (id) do update set name = excluded.name",
        )
        .bind(name.id)
        .bind(&name.name)
        .execute(pool)
        .await?;
        report.names += 1;
    }

    if let Some(error) = listing_error {
        tracing::info!(?report, "SKINR designs updated despite listing failure");
        return Err(error.into());
    }
    Ok(report)
}

async fn store_listing(pool: &PgPool, listing: &Listing) -> sqlx::Result<()> {
    sqlx::query(
        "insert into skinr_listings (id, skinr_id, seller_id, state, price_plex, quantity,
            created, expires, last_modified)
         values ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz, $9::timestamptz)
         on conflict (id) do update set state = excluded.state, price_plex = excluded.price_plex,
            quantity = excluded.quantity, expires = excluded.expires,
            last_modified = excluded.last_modified",
    )
    .bind(&listing.id)
    .bind(&listing.skinr_id)
    .bind(listing.seller_id)
    .bind(&listing.state)
    .bind(listing.price_plex())
    .bind(listing.quantity)
    .bind(&listing.created)
    .bind(&listing.expires)
    .bind(&listing.last_modified)
    .execute(pool)
    .await?;
    Ok(())
}

async fn store_design(pool: &PgPool, design: &Design) -> sqlx::Result<()> {
    sqlx::query(
        "insert into skinr_designs (id, name, line, creator_id, ship_type_id, tier, layout)
         values ($1, $2, $3, $4, $5, $6, $7)
         on conflict (id) do nothing",
    )
    .bind(&design.id)
    .bind(&design.name)
    .bind(&design.line)
    .bind(design.creator_id)
    .bind(design.ship_type_id)
    .bind(design.tier.as_ref().map(|tier| tier.level))
    .bind(design.layout.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Runs [`sync`] forever, each run starting [`SYNC_INTERVAL`] after the last
/// one finished (ESI asks for spaced runs rather than fixed-clock bursts).
pub fn spawn(pool: PgPool, esi: EsiClient) {
    tokio::spawn(async move {
        loop {
            match sync(&pool, &esi).await {
                Ok(report) => tracing::info!(?report, "SKINR sync finished"),
                Err(error) => tracing::warn!(%error, "SKINR sync failed"),
            }
            tokio::time::sleep(SYNC_INTERVAL).await;
        }
    });
}
