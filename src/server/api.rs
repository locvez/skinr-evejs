//! The JSON API the frontend reads ship data from.

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::PgPool;

use super::AppState;
use crate::eve_html::{self, Segment};
use crate::sheet::{self, Sheet};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ships", get(catalog))
        .route("/ships/{id}", get(ship))
        .route("/logos/{file}", get(logo))
        .route("/icons/{file}", get(attribute_icon))
        .route("/skins/{file}", get(skin_icon))
        .route("/skins/layers/{file}", get(skin_icon_layer))
        .route("/ships/{id}/skinrs", get(super::skinr::ship_skinrs))
        .route("/skinr/icons/{file}", get(super::skinr::component_icon))
        .route("/skinr/evejs-target", get(super::imports::export_target))
        .route(
            "/skinr/imports",
            get(super::imports::status).post(super::imports::import_folder),
        )
        .route(
            "/skinr/imports/upload",
            post(super::imports::upload)
                .layer(DefaultBodyLimit::max(super::imports::MAX_JSON_BYTES)),
        )
        .route("/thumbnails/{file}", get(thumbnail))
        .fallback(not_found)
}

#[derive(Serialize)]
struct ApiError {
    message: String,
}

pub(crate) fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ApiError {
            message: message.to_owned(),
        }),
    )
        .into_response()
}

pub(super) fn database_error(cause: sqlx::Error) -> Response {
    tracing::error!(error = %cause, "database query failed");
    error(StatusCode::INTERNAL_SERVER_ERROR, "Database error.")
}

pub async fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, "Not found.")
}

/// A livery's swatch colors as `#rrggbb`.
#[derive(Serialize)]
struct Colors {
    primary: Option<String>,
    secondary: Option<String>,
    hull: Option<String>,
}

/// A faction and the URL of its emblem, when the import extracted one.
#[derive(Serialize, sqlx::FromRow)]
struct FactionInfo {
    id: i32,
    name: String,
    logo_url: Option<String>,
    /// The race this faction is the home empire of, if it is one.
    empire_of: Option<i32>,
}

/// A ship's faction in full: its emblem, motto and description as plain text.
#[derive(Serialize)]
struct FactionDetail {
    id: i32,
    name: String,
    logo_url: Option<String>,
    tagline: Option<String>,
    description: String,
}

#[derive(sqlx::FromRow)]
struct FactionDetailRow {
    id: i32,
    name: String,
    logo_url: Option<String>,
    tagline: Option<String>,
    description: String,
}

/// Faction logos change only with a client patch; a day is plenty.
const LOGO_CACHE_CONTROL: &str = "public, max-age=86400";

/// The Jove Empire, whose emblem the game client does not ship.
const JOVE_EMPIRE: i32 = 500005;
/// Its emblem from Wikimedia Commons, served by the frontend's static files.
const JOVE_EMPIRE_LOGO: &str = "/logos/jove_logo.svg";

/// Selects factions with `logo_url` for those whose logo is stored, or that
/// have one of our own.
fn faction_select() -> String {
    format!(
        "select f.id, f.name, f.empire_of,
            case
                when l.name is not null then '/api/logos/' || l.name || '.png'
                when f.id = {JOVE_EMPIRE} then '{JOVE_EMPIRE_LOGO}'
            end as logo_url
         from factions f left join faction_logos l on l.name = f.logo"
    )
}

/// A ship's Space Object Factory DNA, `hull:faction:race`.
fn dna(hull: &str, faction: &str, race: &str) -> String {
    format!("{hull}:{faction}:{race}")
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    sde_build: Option<String>,
}

async fn health(State(pool): State<PgPool>) -> Result<Json<Health>, Response> {
    let sde_build = crate::db::seeded_sde_build(&pool)
        .await
        .map_err(database_error)?;
    Ok(Json(Health {
        status: "ok",
        sde_build,
    }))
}

#[derive(Serialize, sqlx::FromRow)]
struct Named {
    id: i32,
    name: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct MetaGroup {
    id: i32,
    name: String,
    color: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ShipRow {
    id: i32,
    name: String,
    group_id: i32,
    race_id: Option<i32>,
    faction_id: Option<i32>,
    meta_group_id: Option<i32>,
    tech_level: Option<i32>,
    sof_hull: String,
    sof_faction: String,
    sof_race: String,
    color_primary: Option<String>,
    color_secondary: Option<String>,
    color_hull: Option<String>,
}

#[derive(Serialize)]
struct ShipSummary {
    id: i32,
    name: String,
    group_id: i32,
    race_id: Option<i32>,
    faction_id: Option<i32>,
    meta_group_id: Option<i32>,
    tech_level: Option<i32>,
    dna: String,
    colors: Colors,
}

#[derive(Serialize)]
struct Catalog {
    sde_build: Option<String>,
    races: Vec<Named>,
    factions: Vec<FactionInfo>,
    groups: Vec<Named>,
    meta_groups: Vec<MetaGroup>,
    ships: Vec<ShipSummary>,
}

/// Every ship with the lookups the browser groups them by; small enough
/// (a few hundred rows) to send whole and filter client-side.
async fn catalog(State(pool): State<PgPool>) -> Result<Json<Catalog>, Response> {
    let races = sqlx::query_as::<_, Named>(
        "select id, name from races where id in (select race_id from ships) order by name",
    )
    .fetch_all(&pool)
    .await
    .map_err(database_error)?;

    let groups = sqlx::query_as::<_, Named>("select id, name from ship_groups order by name")
        .fetch_all(&pool)
        .await
        .map_err(database_error)?;

    let meta_groups = sqlx::query_as::<_, MetaGroup>(
        "select id, name, color from meta_groups where id in (select meta_group_id from ships) order by id",
    )
    .fetch_all(&pool)
    .await
    .map_err(database_error)?;

    let ships = sqlx::query_as::<_, ShipRow>(
        "select id, name, group_id, race_id, faction_id, meta_group_id, tech_level, sof_hull, sof_faction, sof_race,
                color_primary, color_secondary, color_hull
         from ships order by name",
    )
    .fetch_all(&pool)
    .await
    .map_err(database_error)?
    .into_iter()
    .map(|row| ShipSummary {
        dna: dna(&row.sof_hull, &row.sof_faction, &row.sof_race),
        id: row.id,
        name: row.name,
        group_id: row.group_id,
        race_id: row.race_id,
        faction_id: row.faction_id,
        meta_group_id: row.meta_group_id,
        tech_level: row.tech_level,
        colors: Colors {
            primary: row.color_primary,
            secondary: row.color_secondary,
            hull: row.color_hull,
        },
    })
    .collect();

    let factions = sqlx::query_as::<_, FactionInfo>(sqlx::AssertSqlSafe(format!(
        "{} where f.id in (select faction_id from ships) order by f.name",
        faction_select()
    )))
    .fetch_all(&pool)
    .await
    .map_err(database_error)?;

    let sde_build = crate::db::seeded_sde_build(&pool)
        .await
        .map_err(database_error)?;

    Ok(Json(Catalog {
        sde_build,
        races,
        factions,
        groups,
        meta_groups,
        ships,
    }))
}

#[derive(sqlx::FromRow)]
struct ShipDetailRow {
    id: i32,
    name: String,
    description: String,
    group_id: i32,
    group_name: String,
    race_id: Option<i32>,
    race_name: Option<String>,
    faction_id: Option<i32>,
    meta_group_id: Option<i32>,
    meta_group_name: Option<String>,
    meta_group_color: Option<String>,
    tech_level: Option<i32>,
    mass: Option<f64>,
    volume: Option<f64>,
    capacity: Option<f64>,
    radius: Option<f64>,
    sof_hull: String,
    sof_faction: String,
    sof_race: String,
    color_primary: Option<String>,
    color_secondary: Option<String>,
    color_hull: Option<String>,
}

#[derive(Serialize)]
struct Skin {
    id: i32,
    name: String,
    dna: String,
    colors: Colors,
    /// The game's icon for the skin material, when the client has one.
    icon_url: Option<String>,
    /// That icon's colors (primary, secondary, hull, lights), so it can be
    /// drawn sharp as vectors.
    icon_colors: Option<serde_json::Value>,
}

#[derive(sqlx::FromRow)]
struct SkinRow {
    skin_id: i32,
    material_id: Option<i32>,
    has_icon: bool,
    icon_colors: Option<String>,
    name: String,
    sof_faction: String,
    dna_suffix: String,
    color_primary: Option<String>,
    color_secondary: Option<String>,
    color_hull: Option<String>,
}

#[derive(Serialize)]
struct ShipDetail {
    id: i32,
    name: String,
    /// The description as plain text.
    description: String,
    /// The description as styled segments, rendered as text by the frontend;
    /// links survive only where they point at a ship with a page here.
    description_rich: Vec<Segment>,
    group: Named,
    race: Option<Named>,
    faction: Option<FactionDetail>,
    meta_group: Option<MetaGroup>,
    tech_level: Option<i32>,
    mass: Option<f64>,
    volume: Option<f64>,
    capacity: Option<f64>,
    radius: Option<f64>,
    dna: String,
    colors: Colors,
    skins: Vec<Skin>,
    sheet: Sheet,
    bonuses: Vec<BonusGroup>,
}

/// Bonus lines under one heading: a skill ("per skill level"), the role
/// bonus, or the SDE's miscellaneous ones.
#[derive(Serialize)]
struct BonusGroup {
    title: String,
    per_level: bool,
    lines: Vec<BonusLine>,
}

#[derive(Serialize)]
struct BonusLine {
    /// "5%", "-10%", "2x"; absent for lines without a figure.
    amount: Option<String>,
    text: Vec<Segment>,
}

#[derive(sqlx::FromRow)]
struct BonusRow {
    kind: String,
    skill_name: Option<String>,
    bonus: Option<f64>,
    unit: Option<String>,
    text: String,
}

/// A bonus figure with its unit the way the game prints it: "5%", "2x", "10 m".
fn bonus_amount(bonus: Option<f64>, unit: Option<&str>) -> Option<String> {
    let bonus = bonus?;
    let number = if bonus.fract() == 0.0 {
        format!("{bonus:.0}")
    } else {
        format!("{bonus}")
    };
    Some(match unit {
        None | Some("") => number,
        Some(unit @ ("%" | "x")) => format!("{number}{unit}"),
        Some(unit) => format!("{number} {unit}"),
    })
}

/// Groups bonus rows (already in display order) under their headings.
fn group_bonuses(rows: Vec<BonusRow>) -> Vec<BonusGroup> {
    let mut groups: Vec<BonusGroup> = Vec::new();
    for row in rows {
        let (title, per_level) = match row.kind.as_str() {
            "skill" => (
                format!("{} bonuses", row.skill_name.as_deref().unwrap_or("Skill")),
                true,
            ),
            "role" => ("Role bonus".to_owned(), false),
            _ => ("Characteristics".to_owned(), false),
        };
        // Bonus text links skills and modules, which have no page here.
        let mut text = eve_html::parse(&row.text);
        for segment in &mut text {
            if let Segment::Text(span) = segment {
                span.type_id = None;
            }
        }
        let line = BonusLine {
            amount: bonus_amount(row.bonus, row.unit.as_deref()),
            text,
        };
        match groups.last_mut() {
            Some(group) if group.title == title => group.lines.push(line),
            _ => groups.push(BonusGroup {
                title,
                per_level,
                lines: vec![line],
            }),
        }
    }
    groups
}

async fn ship(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<ShipDetail>, Response> {
    let row = sqlx::query_as::<_, ShipDetailRow>(
        "select s.id, s.name, s.description, s.group_id, g.name as group_name,
                s.race_id, r.name as race_name, s.faction_id,
                s.meta_group_id, m.name as meta_group_name, m.color as meta_group_color,
                s.tech_level, s.mass, s.volume, s.capacity, s.radius,
                s.sof_hull, s.sof_faction, s.sof_race,
                s.color_primary, s.color_secondary, s.color_hull
         from ships s
         join ship_groups g on g.id = s.group_id
         left join races r on r.id = s.race_id
         left join meta_groups m on m.id = s.meta_group_id
         where s.id = $1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(database_error)?
    .ok_or_else(|| error(StatusCode::NOT_FOUND, "Ship not found."))?;

    let skins = sqlx::query_as::<_, SkinRow>(
        "select skin_id, ship_skins.material_id, name, sof_faction, dna_suffix,
                color_primary, color_secondary, color_hull,
                skin_icons.material_id is not null as has_icon, skin_icons.colors as icon_colors
         from ship_skins left join skin_icons using (material_id)
         where ship_id = $1 order by name, skin_id",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?
    .into_iter()
    .map(|skin| Skin {
        id: skin.skin_id,
        name: skin.name,
        dna: format!(
            "{}{}",
            dna(&row.sof_hull, &skin.sof_faction, &row.sof_race),
            skin.dna_suffix
        ),
        colors: Colors {
            primary: skin.color_primary,
            secondary: skin.color_secondary,
            hull: skin.color_hull,
        },
        icon_url: skin
            .material_id
            .filter(|_| skin.has_icon)
            .map(|material| format!("/api/skins/{material}.png")),
        icon_colors: skin
            .icon_colors
            .as_deref()
            .and_then(|colors| serde_json::from_str(colors).ok()),
    })
    .collect();

    let description_rich = linkable(&pool, eve_html::parse(&row.description)).await?;

    let attributes: std::collections::HashMap<String, f64> = sqlx::query_as::<_, (String, f64)>(
        "select name, value from ship_attributes where ship_id = $1",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?
    .into_iter()
    .collect();
    let sheet = sheet::build(&attributes, row.mass);

    let bonuses = group_bonuses(
        sqlx::query_as::<_, BonusRow>(
            "select kind, skill_name, bonus, unit, text from ship_bonuses
             where ship_id = $1 order by position",
        )
        .bind(id)
        .fetch_all(&pool)
        .await
        .map_err(database_error)?,
    );
    let faction = match row.faction_id {
        Some(faction_id) => sqlx::query_as::<_, FactionDetailRow>(sqlx::AssertSqlSafe(format!(
            "select f.id, f.name, faction.logo_url, f.tagline, f.description
             from factions f join ({}) faction using (id) where f.id = $1",
            faction_select()
        )))
        .bind(faction_id)
        .fetch_optional(&pool)
        .await
        .map_err(database_error)?
        .map(|faction| FactionDetail {
            id: faction.id,
            name: faction.name,
            logo_url: faction.logo_url,
            tagline: faction
                .tagline
                .map(|tagline| eve_html::plain_text(&tagline).trim().to_owned()),
            description: eve_html::plain_text(&faction.description).trim().to_owned(),
        }),
        None => None,
    };
    Ok(Json(ShipDetail {
        dna: dna(&row.sof_hull, &row.sof_faction, &row.sof_race),
        id: row.id,
        name: row.name,
        description: eve_html::plain_text(&row.description).trim().to_owned(),
        description_rich,
        group: Named {
            id: row.group_id,
            name: row.group_name,
        },
        race: row
            .race_id
            .zip(row.race_name)
            .map(|(id, name)| Named { id, name }),
        faction,
        meta_group: row
            .meta_group_id
            .zip(row.meta_group_name)
            .map(|(id, name)| MetaGroup {
                id,
                name,
                color: row.meta_group_color,
            }),
        tech_level: row.tech_level,
        mass: row.mass,
        volume: row.volume,
        capacity: row.capacity,
        radius: row.radius,
        colors: Colors {
            primary: row.color_primary,
            secondary: row.color_secondary,
            hull: row.color_hull,
        },
        skins,
        sheet,
        bonuses,
    }))
}

/// Drops every link that does not point at a ship this site has a page for.
async fn linkable(pool: &PgPool, mut segments: Vec<Segment>) -> Result<Vec<Segment>, Response> {
    let linked: Vec<i32> = segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Text(span) => span.type_id,
            Segment::Break { .. } => None,
        })
        .collect();
    if linked.is_empty() {
        return Ok(segments);
    }
    let ships: Vec<i32> = sqlx::query_scalar("select id from ships where id = any($1)")
        .bind(&linked)
        .fetch_all(pool)
        .await
        .map_err(database_error)?;
    for segment in &mut segments {
        if let Segment::Text(span) = segment
            && span.type_id.is_some_and(|id| !ships.contains(&id))
        {
            span.type_id = None;
        }
    }
    Ok(segments)
}

/// A faction emblem as the import stored it, by `<flatLogo>.png`.
async fn logo(State(pool): State<PgPool>, Path(file): Path<String>) -> Result<Response, Response> {
    let Some(name) = file.strip_suffix(".png") else {
        return Err(not_found().await);
    };
    let png: Option<Vec<u8>> = sqlx::query_scalar("select png from faction_logos where name = $1")
        .bind(name)
        .fetch_optional(&pool)
        .await
        .map_err(database_error)?;
    png_response(png, "Logo not found.")
}

/// A stored client image as a long-cached PNG, or a 404 saying what is missing.
pub(super) fn png_response(png: Option<Vec<u8>>, missing: &str) -> Result<Response, Response> {
    let Some(png) = png else {
        return Err(error(StatusCode::NOT_FOUND, missing));
    };
    Ok((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/png")),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(LOGO_CACHE_CONTROL),
            ),
        ],
        png,
    )
        .into_response())
}

/// Thumbnails are re-rendered when the look changes, so they are cached briefly.
const THUMBNAIL_CACHE_CONTROL: &str = "public, max-age=300";
/// The themes a thumbnail is rendered in.
const THUMBNAIL_THEMES: [&str; 2] = ["dark", "light"];

/// A ship's rendered thumbnail, by `<type id>-<theme>.png`. Only names of that
/// shape are read, so a request can never reach outside the directory.
async fn thumbnail(State(state): State<AppState>, Path(file): Path<String>) -> Response {
    let valid = file
        .strip_suffix(".png")
        .and_then(|name| name.split_once('-'))
        .is_some_and(|(id, theme)| id.parse::<u32>().is_ok() && THUMBNAIL_THEMES.contains(&theme));
    if !valid {
        return not_found().await;
    }
    match tokio::fs::read(state.thumbnails.join(&file)).await {
        Ok(png) => (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static("image/png")),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(THUMBNAIL_CACHE_CONTROL),
                ),
            ],
            png,
        )
            .into_response(),
        Err(_) => error(StatusCode::NOT_FOUND, "Thumbnail not found."),
    }
}

/// A sheet attribute's in-game icon, by `<attribute name>.png`.
async fn attribute_icon(
    State(pool): State<PgPool>,
    Path(file): Path<String>,
) -> Result<Response, Response> {
    let Some(name) = file.strip_suffix(".png") else {
        return Err(not_found().await);
    };
    let png: Option<Vec<u8>> =
        sqlx::query_scalar("select png from attribute_icons where name = $1")
            .bind(name)
            .fetch_optional(&pool)
            .await
            .map_err(database_error)?;
    png_response(png, "Icon not found.")
}

/// A skin material's in-game icon, by `<material id>.png`.
async fn skin_icon(
    State(pool): State<PgPool>,
    Path(file): Path<String>,
) -> Result<Response, Response> {
    let Some(material) = file
        .strip_suffix(".png")
        .and_then(|id| id.parse::<i32>().ok())
    else {
        return Err(not_found().await);
    };
    let png: Option<Vec<u8>> =
        sqlx::query_scalar("select png from skin_icons where material_id = $1")
            .bind(material)
            .fetch_optional(&pool)
            .await
            .map_err(database_error)?;
    png_response(png, "Icon not found.")
}

/// A layer of the client's tintable skin icon, by `<layer>.png`.
async fn skin_icon_layer(
    State(pool): State<PgPool>,
    Path(file): Path<String>,
) -> Result<Response, Response> {
    let Some(name) = file.strip_suffix(".png") else {
        return Err(not_found().await);
    };
    let png: Option<Vec<u8>> =
        sqlx::query_scalar("select png from skin_icon_layers where name = $1")
            .bind(name)
            .fetch_optional(&pool)
            .await
            .map_err(database_error)?;
    png_response(png, "Layer not found.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonus_amounts_print_like_the_game() {
        assert_eq!(bonus_amount(Some(5.0), Some("%")).as_deref(), Some("5%"));
        assert_eq!(
            bonus_amount(Some(-10.0), Some("%")).as_deref(),
            Some("-10%")
        );
        assert_eq!(
            bonus_amount(Some(37.5), Some("%")).as_deref(),
            Some("37.5%")
        );
        assert_eq!(bonus_amount(Some(2.0), Some("x")).as_deref(), Some("2x"));
        assert_eq!(bonus_amount(Some(10.0), Some("m")).as_deref(), Some("10 m"));
        assert_eq!(bonus_amount(Some(3.0), None).as_deref(), Some("3"));
        assert_eq!(bonus_amount(None, Some("%")), None);
    }

    #[test]
    fn bonuses_group_under_their_heading() {
        let row = |kind: &str, skill: Option<&str>, text: &str| BonusRow {
            kind: kind.into(),
            skill_name: skill.map(Into::into),
            bonus: Some(5.0),
            unit: Some("%".into()),
            text: text.into(),
        };
        let groups = group_bonuses(vec![
            row("skill", Some("Caldari Frigate"), "a"),
            row("skill", Some("Caldari Frigate"), "b"),
            row("role", None, "c"),
        ]);
        let shape: Vec<_> = groups
            .iter()
            .map(|group| (group.title.as_str(), group.per_level, group.lines.len()))
            .collect();
        assert_eq!(
            shape,
            vec![
                ("Caldari Frigate bonuses", true, 2),
                ("Role bonus", false, 1)
            ]
        );
    }
}
