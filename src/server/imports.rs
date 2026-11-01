//! Explicit, transactional import of local ESI/SKINR JSON snapshots.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path as FilePath, PathBuf};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use super::api::{database_error, error};
use crate::esi::skinr::Design;

/// Large enough for the provided design snapshots, bounded for browser uploads.
pub const MAX_JSON_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_EVEJS_PROFILE: &str = "storage/evejs/export-target.json";
const DEFAULT_JSON_DIR: &str = "src/skinrJSON";

#[derive(Serialize)]
pub struct SourceFile {
    name: String,
    bytes: u64,
    kind: &'static str,
}

#[derive(Serialize)]
pub struct ImportStatus {
    folder_available: bool,
    files: Vec<SourceFile>,
    imported_designs: i64,
    ships: Vec<ImportedShip>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ImportedShip {
    id: i32,
    name: String,
    designs: i64,
}

#[derive(Default, Deserialize)]
pub struct FolderRequest {
    /// Empty imports all design snapshots. Only names returned by GET are accepted.
    #[serde(default)]
    files: Vec<String>,
}

#[derive(Serialize)]
pub struct ImportReport {
    pub files: usize,
    pub designs: usize,
    pub added: u64,
    pub already_imported: u64,
}

fn source_dir() -> PathBuf {
    std::env::var("SKINR_JSON_DIR")
        .unwrap_or_else(|_| DEFAULT_JSON_DIR.to_owned())
        .into()
}

fn source_files(dir: &FilePath) -> std::io::Result<Vec<SourceFile>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".json") {
            continue;
        }
        files.push(SourceFile {
            kind: if name.starts_with("skinr-listings-") {
                "listings"
            } else {
                "designs"
            },
            name,
            bytes: entry.metadata()?.len(),
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

pub async fn status(State(pool): State<PgPool>) -> Result<Json<ImportStatus>, Response> {
    let dir = source_dir();
    let files = source_files(&dir).map_err(|cause| {
        tracing::warn!(%cause, "could not list JSON imports");
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The JSON folder could not be read.",
        )
    })?;
    let imported_designs = sqlx::query_scalar("select count(*) from skinr_imports")
        .fetch_one(&pool)
        .await
        .map_err(database_error)?;
    let ships = sqlx::query_as::<_, ImportedShip>(
        "select s.id, s.name, count(*) designs from skinr_imports i
         join skinr_designs d on d.id=i.skinr_id join ships s on s.id=d.ship_type_id
         group by s.id order by s.name",
    )
    .fetch_all(&pool)
    .await
    .map_err(database_error)?;
    Ok(Json(ImportStatus {
        folder_available: dir.is_dir(),
        files,
        imported_designs,
        ships,
    }))
}

fn invalid(message: impl AsRef<str>) -> Response {
    error(StatusCode::UNPROCESSABLE_ENTITY, message.as_ref())
}

pub async fn export_target() -> Result<Json<Option<Value>>, Response> {
    let path =
        std::env::var("EVEJS_EXPORT_PROFILE").unwrap_or_else(|_| DEFAULT_EVEJS_PROFILE.to_owned());
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => return Ok(Json(None)),
        Err(_) => {
            return Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The EveOffline export profile could not be read.",
            ));
        }
    };
    let profile: Value = serde_json::from_slice(&bytes).map_err(|_| {
        invalid("Invalid EveOffline export profile. Regenerate the target snapshot.")
    })?;
    if profile["format"] != "eveoffline.skinr-export-target" || profile["version"] != 1 {
        return Err(invalid(
            "Unsupported EveOffline export profile. Regenerate the target snapshot.",
        ));
    }
    Ok(Json(Some(profile)))
}

/// EVE Ref maps, individual ESI designs, arrays, and versioned export bundles.
pub fn parse_designs(value: Value) -> Result<BTreeMap<String, Design>, String> {
    let value = if value.get("format").and_then(Value::as_str) == Some("evejs-skinr") {
        if value.get("version").and_then(Value::as_u64) != Some(1) {
            return Err("Unsupported EveJS export version.".into());
        }
        value
            .get("designs")
            .cloned()
            .ok_or("The export has no designs.")?
    } else {
        value
    };
    if value.get("listings").is_some() && value.get("ship_type_id").is_none() {
        return Err("This is a listing snapshot. Import its skinr-details file instead.".into());
    }
    let entries: Vec<(Option<String>, Value)> = match value {
        Value::Array(rows) => rows.into_iter().map(|row| (None, row)).collect(),
        Value::Object(map) if map.contains_key("ship_type_id") => vec![(None, Value::Object(map))],
        Value::Object(map) => map.into_iter().map(|(key, row)| (Some(key), row)).collect(),
        _ => {
            return Err(
                "Expected a design, an array of designs, or a map keyed by design ID.".into(),
            );
        }
    };
    if entries.is_empty() {
        return Err("The JSON contains no designs.".into());
    }
    let mut designs = BTreeMap::new();
    for (key, value) in entries {
        let design: Design =
            serde_json::from_value(value).map_err(|cause| format!("Invalid design: {cause}"))?;
        if key.as_ref().is_some_and(|key| key != &design.id) {
            return Err(format!("Design map key does not match {}.", design.id));
        }
        if design.id.is_empty()
            || design.id.len() > 128
            || !design.id.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Design IDs must contain at most 128 hexadecimal characters.".into());
        }
        if design.name.trim().is_empty() || design.ship_type_id <= 0 || design.creator_id <= 0 {
            return Err(format!(
                "Design {} requires a name, ship type, and creator.",
                design.id
            ));
        }
        validate_layout(&design.layout).map_err(|message| format!("{}: {message}", design.name))?;
        if let Some(previous) = designs.insert(design.id.clone(), design) {
            let current = &designs[&previous.id];
            if serde_json::to_value(previous).unwrap() != serde_json::to_value(current).unwrap() {
                return Err("Conflicting designs share an ID.".into());
            }
        }
    }
    Ok(designs)
}

fn validate_layout(layout: &Value) -> Result<(), String> {
    let slots = layout
        .get("slots")
        .and_then(Value::as_array)
        .ok_or("Layout requires a slots array.")?;
    let mut seen = BTreeSet::new();
    for slot in slots {
        let id = slot
            .get("id")
            .and_then(Value::as_i64)
            .ok_or("Slot ID must be an integer.")?;
        if !(1..=8).contains(&id) || !seen.insert(id) {
            return Err("Slot IDs must be unique and between 1 and 8.".into());
        }
        let configuration = slot
            .get("configuration")
            .and_then(Value::as_object)
            .ok_or("Slot configuration must be an object.")?;
        for (kind, part) in configuration {
            if !["nanocoating", "pattern"].contains(&kind.as_str()) {
                return Err(format!("Unknown slot configuration: {kind}."));
            }
            if part
                .get("id")
                .and_then(Value::as_i64)
                .is_none_or(|id| id <= 0)
            {
                return Err("Component ID must be a positive integer.".into());
            }
            let pattern_slot = id == 5 || id == 7;
            if (kind == "pattern") != pattern_slot {
                return Err("Component is in the wrong slot.".into());
            }
            if kind != "pattern" {
                continue;
            }
            let pattern = part
                .get("configuration")
                .and_then(Value::as_object)
                .ok_or("Pattern configuration must be an object.")?;
            if pattern
                .get("mirrored")
                .is_some_and(|value| !value.is_boolean())
            {
                return Err("Pattern mirrored must be boolean.".into());
            }
            if let Some(projection) = pattern.get("projection") {
                let projection = projection
                    .as_object()
                    .ok_or("Pattern projection must be an object.")?;
                if projection.iter().any(|(key, value)| {
                    !["slot1", "slot2", "slot3", "slot4"].contains(&key.as_str())
                        || !value.is_boolean()
                }) {
                    return Err(
                        "Pattern projection requires boolean slot1 through slot4 values.".into(),
                    );
                }
            }
            if let Some(transform) = pattern.get("transform") {
                for (field, axes) in [
                    ("position", "xyz"),
                    ("rotation", "xyzw"),
                    ("scaling", "xyz"),
                ] {
                    for axis in axes.chars() {
                        if transform
                            .get(field)
                            .and_then(|v| v.get(axis.to_string()))
                            .and_then(Value::as_f64)
                            .is_none()
                        {
                            return Err(format!("Pattern {field}.{axis} must be numeric."));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn store_designs(
    pool: &PgPool,
    designs: BTreeMap<String, (Design, String)>,
    files: usize,
) -> Result<ImportReport, Response> {
    let ships: BTreeSet<i32> = sqlx::query_scalar("select id from ships")
        .fetch_all(pool)
        .await
        .map_err(database_error)?
        .into_iter()
        .collect();
    let components: BTreeMap<i32, String> =
        sqlx::query_as::<_, (i32, String)>("select id,category from skinr_components")
            .fetch_all(pool)
            .await
            .map_err(database_error)?
            .into_iter()
            .collect();
    for (design, _) in designs.values() {
        validate_layout(&design.layout).map_err(invalid)?;
        if !ships.contains(&design.ship_type_id) {
            return Err(invalid(format!(
                "{} uses ship type {}, which is not in the catalogue.",
                design.name, design.ship_type_id
            )));
        }
        for slot in design.layout["slots"].as_array().unwrap() {
            for kind in ["nanocoating", "pattern"] {
                if let Some(id) = slot["configuration"][kind]["id"].as_i64() {
                    let category = i32::try_from(id).ok().and_then(|id| components.get(&id));
                    if category
                        .is_none_or(|category| (category == "Pattern") != (kind == "pattern"))
                    {
                        return Err(invalid(format!(
                            "{} uses unknown or incompatible component {id}. Refresh the SDE before importing.",
                            design.name
                        )));
                    }
                }
            }
        }
    }
    let mut tx = pool.begin().await.map_err(database_error)?;
    let mut report = ImportReport {
        files,
        designs: designs.len(),
        added: 0,
        already_imported: 0,
    };
    for (id, (design, source)) in designs {
        // Never overwrite an existing, immutable ESI design or create fake listings.
        let stored = sqlx::query(
            "insert into skinr_designs (id,name,line,creator_id,ship_type_id,tier,layout)
            values ($1,$2,$3,$4,$5,$6,$7) on conflict (id) do update set id=excluded.id
            where skinr_designs.name=excluded.name and skinr_designs.line is not distinct from excluded.line
                and skinr_designs.creator_id=excluded.creator_id and skinr_designs.ship_type_id=excluded.ship_type_id
                and skinr_designs.tier is not distinct from excluded.tier
                and skinr_designs.layout::jsonb=excluded.layout::jsonb",
        )
        .bind(&id)
        .bind(&design.name)
        .bind(&design.line)
        .bind(design.creator_id)
        .bind(design.ship_type_id)
        .bind(design.tier.map(|tier| tier.level))
        .bind(design.layout.to_string())
        .execute(&mut *tx)
        .await
        .map_err(database_error)?.rows_affected();
        if stored == 0 {
            return Err(invalid(format!(
                "{} conflicts with an existing immutable design. No designs were imported.",
                design.name
            )));
        }
        let added = sqlx::query("insert into skinr_imports (skinr_id,source_file) values ($1,$2) on conflict (skinr_id) do nothing")
            .bind(&id).bind(source).execute(&mut *tx).await.map_err(database_error)?.rows_affected();
        report.added += added;
        report.already_imported += 1 - added;
    }
    tx.commit().await.map_err(database_error)?;
    Ok(report)
}

pub async fn import_folder(
    State(pool): State<PgPool>,
    Json(request): Json<FolderRequest>,
) -> Result<Json<ImportReport>, Response> {
    let dir = source_dir();
    let available =
        source_files(&dir).map_err(|_| invalid("The JSON folder could not be read."))?;
    let selected: Vec<_> = if request.files.is_empty() {
        available
            .iter()
            .filter(|file| file.kind == "designs")
            .collect()
    } else {
        request
            .files
            .iter()
            .map(|name| {
                available
                    .iter()
                    .find(|file| &file.name == name)
                    .ok_or_else(|| invalid("Choose a file listed in the JSON folder."))
            })
            .collect::<Result<_, _>>()?
    };
    if selected.is_empty() {
        return Err(invalid("No design JSON files were found in the folder."));
    }
    let mut designs = BTreeMap::new();
    for file in &selected {
        if file.bytes > MAX_JSON_BYTES as u64 {
            return Err(invalid(format!(
                "{} exceeds the 32 MiB file limit.",
                file.name
            )));
        }
        let bytes = std::fs::read(dir.join(&file.name))
            .map_err(|_| invalid(format!("{} could not be read.", file.name)))?;
        let value = serde_json::from_slice(&bytes)
            .map_err(|cause| invalid(format!("{}: {cause}", file.name)))?;
        for (id, design) in
            parse_designs(value).map_err(|cause| invalid(format!("{}: {cause}", file.name)))?
        {
            if let Some((previous, _)) = designs.get(&id)
                && serde_json::to_value(previous).unwrap() != serde_json::to_value(&design).unwrap()
            {
                return Err(invalid(format!(
                    "Conflicting snapshots for design {id}. Import these files separately."
                )));
            }
            designs.insert(id, (design, file.name.clone()));
        }
    }
    Ok(Json(store_designs(&pool, designs, selected.len()).await?))
}

pub async fn upload(
    State(pool): State<PgPool>,
    Json(value): Json<Value>,
) -> Result<Json<ImportReport>, Response> {
    let designs = parse_designs(value)
        .map_err(invalid)?
        .into_iter()
        .map(|(id, design)| (id, (design, "Uploaded JSON".to_owned())))
        .collect();
    Ok(Json(store_designs(&pool, designs, 1).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn design() -> Value {
        json!({"id":"abcdef", "name":"Archived look", "creator_id":7,"ship_type_id":601,
            "layout":{"slots":[{"id":1,"configuration":{"nanocoating":{"id":168}}}]}})
    }

    #[test]
    fn reads_maps_single_designs_arrays_and_export_bundles() {
        let row = design();
        for input in [
            row.clone(),
            json!([row.clone(), row.clone()]),
            json!({"abcdef":row.clone()}),
            json!({"format":"evejs-skinr","version":1,"designs":{"abcdef":row.clone()}}),
        ] {
            let parsed = parse_designs(input).unwrap();
            assert_eq!(parsed.len(), 1);
            assert_eq!(parsed["abcdef"].layout, row["layout"]);
        }
    }

    #[test]
    fn rejects_listings_wrong_keys_conflicts_and_unknown_versions() {
        assert!(
            parse_designs(json!({"listings":[]}))
                .unwrap_err()
                .contains("listing snapshot")
        );
        assert!(parse_designs(json!({"badkey":design()})).is_err());
        assert!(parse_designs(json!({"format":"evejs-skinr","version":2,"designs":{}})).is_err());
        let mut changed = design();
        changed["name"] = json!("Different recipe");
        assert!(
            parse_designs(json!([design(), changed]))
                .unwrap_err()
                .contains("Conflicting")
        );
    }

    #[test]
    fn rejects_layouts_that_would_crash_the_viewer() {
        for layout in [
            Value::Null,
            json!({"slots":null}),
            json!({"slots":[{"id":1,"configuration":{"pattern":{"id":5}}}]}),
            json!({"slots":[{"id":5,"configuration":{"pattern":{"id":5,"configuration":{"transform":{"position":{}}}}}}]}),
        ] {
            let mut row = design();
            row["layout"] = layout;
            assert!(parse_designs(row).is_err());
        }
    }
}
