//! Player-made SKINR designs for a ship, from the Paragon Hub sync
//! (`crate::esi::skinr`) joined with the SDE's component tables.

use std::collections::{BTreeMap, BTreeSet};

use axum::Json;
use axum::extract::{Path, State};
use axum::response::Response;
use serde::Serialize;
use sqlx::PgPool;

use super::api::{database_error, not_found, png_response};

/// The listing state of a design that can be bought now.
const LISTED: &str = "listed";

#[derive(Serialize)]
pub struct ShipSkinrs {
    /// For this hull's faction: SKINR material slot (1-4) to Trinity hull
    /// material (1-4). Empty when the SDE has no mapping for the faction.
    slot_materials: BTreeMap<i32, i32>,
    designs: Vec<DesignOut>,
    /// The components the designs use, by id.
    components: BTreeMap<i32, ComponentOut>,
}

#[derive(Serialize)]
struct DesignOut {
    id: String,
    name: String,
    line: Option<String>,
    tier: Option<i32>,
    creator: Person,
    /// The design's slots exactly as ESI returns them.
    layout: serde_json::Value,
    /// Live Paragon Hub listings, their total licenses, and the cheapest.
    listings: i64,
    quantity: i64,
    min_price_plex: Option<i64>,
    imported: bool,
    source_file: Option<String>,
}

#[derive(Serialize)]
struct Person {
    id: i64,
    name: Option<String>,
}

#[derive(Serialize)]
struct ComponentOut {
    id: i32,
    name: String,
    category: String,
    finish: Option<String>,
    /// For materials: the SOF material name DNA uses (`material?…`).
    material: Option<String>,
    /// For patterns: the projection texture.
    texture: Option<String>,
    projection_u: Option<String>,
    projection_v: Option<String>,
    icon_url: Option<String>,
    /// For materials: a swatch color, `#rrggbb`.
    color: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DesignRow {
    id: String,
    name: String,
    line: Option<String>,
    tier: Option<i32>,
    creator_id: i64,
    creator_name: Option<String>,
    layout: String,
    listings: i64,
    quantity: i64,
    min_price_plex: Option<i64>,
    imported: bool,
    source_file: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ComponentRow {
    id: i32,
    name: String,
    category: String,
    finish: Option<String>,
    resource_file: String,
    projection_u: Option<String>,
    projection_v: Option<String>,
    color: Option<String>,
    has_icon: bool,
}

/// The SOF name of a material file: its file name without extension.
fn sof_material_name(resource_file: &str) -> String {
    let file = resource_file.rsplit('/').next().unwrap_or(resource_file);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    stem.to_lowercase()
}

/// Every component id a design's layout uses (nanocoatings and patterns).
fn component_ids(layout: &serde_json::Value, into: &mut BTreeSet<i32>) {
    let Some(slots) = layout.get("slots").and_then(|slots| slots.as_array()) else {
        return;
    };
    for slot in slots {
        let Some(configuration) = slot.get("configuration") else {
            continue;
        };
        for kind in ["nanocoating", "pattern"] {
            if let Some(id) = configuration
                .get(kind)
                .and_then(|part| part.get("id"))
                .and_then(|id| id.as_i64())
            {
                into.insert(id as i32);
            }
        }
    }
}

/// `/api/ships/{id}/skinrs`: the designs listed for a ship and what they are made of.
pub async fn ship_skinrs(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<ShipSkinrs>, Response> {
    let faction_id: Option<Option<i32>> =
        sqlx::query_scalar("select faction_id from ships where id = $1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(database_error)?;
    let Some(faction_id) = faction_id else {
        return Err(not_found().await);
    };

    let slot_materials: BTreeMap<i32, i32> = sqlx::query_as::<_, (i32, i32)>(
        "select slot_id, material_id from skinr_slot_materials where faction_id = $1",
    )
    .bind(faction_id)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?
    .into_iter()
    .collect();

    let rows = sqlx::query_as::<_, DesignRow>(
        "select d.id, d.name, d.line, d.tier, d.creator_id, c.name as creator_name, d.layout,
                count(l.id) as listings, coalesce(sum(l.quantity), 0)::bigint as quantity,
                min(l.price_plex) as min_price_plex,
                i.skinr_id is not null as imported, i.source_file
         from skinr_designs d
         left join skinr_listings l on l.skinr_id = d.id and l.state = $2 and l.expires > now()
         left join skinr_imports i on i.skinr_id = d.id
         left join esi_characters c on c.id = d.creator_id
         where d.ship_type_id = $1 and (l.id is not null or i.skinr_id is not null)
         group by d.id, c.name, i.skinr_id, i.source_file
         order by count(l.id) desc, d.name",
    )
    .bind(id)
    .bind(LISTED)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?;

    let mut used = BTreeSet::new();
    let mut designs = Vec::with_capacity(rows.len());
    for row in rows {
        let layout: serde_json::Value = serde_json::from_str(&row.layout).unwrap_or_default();
        component_ids(&layout, &mut used);
        designs.push(DesignOut {
            id: row.id,
            name: row.name,
            line: row.line,
            tier: row.tier,
            creator: Person {
                id: row.creator_id,
                name: row.creator_name,
            },
            layout,
            listings: row.listings,
            quantity: row.quantity,
            min_price_plex: row.min_price_plex,
            imported: row.imported,
            source_file: row.source_file,
        });
    }

    let ids: Vec<i32> = used.into_iter().collect();
    let components = sqlx::query_as::<_, ComponentRow>(
        "select c.id, c.name, c.category, c.finish, c.resource_file, c.projection_u, c.projection_v,
                c.color,
                i.component_id is not null as has_icon
         from skinr_components c
         left join skinr_component_icons i on i.component_id = c.id
         where c.id = any($1)",
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?
    .into_iter()
    .map(|row| {
        let is_pattern = row.category == "Pattern";
        (
            row.id,
            ComponentOut {
                id: row.id,
                material: (!is_pattern).then(|| sof_material_name(&row.resource_file)),
                texture: is_pattern.then(|| row.resource_file.to_lowercase()),
                icon_url: row.has_icon.then(|| format!("/api/skinr/icons/{}.png", row.id)),
                name: row.name,
                category: row.category,
                finish: row.finish,
                projection_u: row.projection_u,
                projection_v: row.projection_v,
                color: row.color,
            },
        )
    })
    .collect();

    Ok(Json(ShipSkinrs {
        slot_materials,
        designs,
        components,
    }))
}

/// `/api/skinr/icons/{component}.png`: the client's icon for a component.
pub async fn component_icon(
    State(pool): State<PgPool>,
    Path(file): Path<String>,
) -> Result<Response, Response> {
    let Some(id) = file
        .strip_suffix(".png")
        .and_then(|id| id.parse::<i32>().ok())
    else {
        return Err(not_found().await);
    };
    let png: Option<Vec<u8>> =
        sqlx::query_scalar("select png from skinr_component_icons where component_id = $1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(database_error)?;
    png_response(png, "Icon not found.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_files_become_sof_names() {
        assert_eq!(
            sof_material_name(
                "res:/dx9/model/SpaceObjectFactory/materials/cosm_BW_matt_000_000_100.red"
            ),
            "cosm_bw_matt_000_000_100"
        );
    }

    #[test]
    fn layouts_name_their_components() {
        let layout = serde_json::json!({"slots": [
            {"id": 1, "configuration": {"nanocoating": {"id": 1143}}},
            {"id": 5, "configuration": {"pattern": {"id": 188, "configuration": {}}}},
            {"id": 6, "configuration": {"nanocoating": {"id": 209}}}
        ]});
        let mut ids = BTreeSet::new();
        component_ids(&layout, &mut ids);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec![188, 209, 1143]);
    }
}
