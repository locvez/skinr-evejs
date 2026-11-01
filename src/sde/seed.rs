//! Writes [`ShipTables`] into Postgres, replacing the previous build's rows
//! in one transaction so readers never see a half-seeded state.

use sqlx::PgPool;

use super::ShipTables;

pub async fn seed(pool: &PgPool, tables: &ShipTables) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "truncate ship_attributes, ship_bonuses, ship_skins, ships, ship_groups, meta_groups, races, factions,
            skinr_components, skinr_slot_materials",
    )
        .execute(&mut *tx)
        .await?;

    for race in &tables.races {
        sqlx::query("insert into races (id, name, description) values ($1, $2, $3)")
            .bind(race.id)
            .bind(&race.name)
            .bind(&race.description)
            .execute(&mut *tx)
            .await?;
    }

    for faction in &tables.factions {
        sqlx::query(
            "insert into factions (id, name, logo, description, tagline, empire_of)
             values ($1, $2, $3, $4, $5, $6)",
        )
        .bind(faction.id)
        .bind(&faction.name)
        .bind(&faction.logo)
        .bind(&faction.description)
        .bind(&faction.tagline)
        .bind(faction.empire_of)
        .execute(&mut *tx)
        .await?;
    }

    for meta in &tables.meta_groups {
        sqlx::query("insert into meta_groups (id, name, color) values ($1, $2, $3)")
            .bind(meta.id)
            .bind(&meta.name)
            .bind(&meta.color)
            .execute(&mut *tx)
            .await?;
    }

    for group in &tables.groups {
        sqlx::query("insert into ship_groups (id, name) values ($1, $2)")
            .bind(group.id)
            .bind(&group.name)
            .execute(&mut *tx)
            .await?;
    }

    for ship in &tables.ships {
        sqlx::query(
            "insert into ships (id, name, description, group_id, race_id, meta_group_id,
                tech_level, mass, volume, capacity, radius, sof_hull, sof_faction, sof_race,
                color_primary, color_secondary, color_hull, faction_id)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
        )
        .bind(ship.id)
        .bind(&ship.name)
        .bind(&ship.description)
        .bind(ship.group_id)
        // A race or meta group the SDE does not list (or none) is stored as null.
        .bind(
            ship.race_id
                .filter(|id| tables.races.iter().any(|race| race.id == *id)),
        )
        .bind(
            ship.meta_group_id
                .filter(|id| tables.meta_groups.iter().any(|meta| meta.id == *id)),
        )
        .bind(ship.tech_level)
        .bind(ship.mass)
        .bind(ship.volume)
        .bind(ship.capacity)
        .bind(ship.radius)
        .bind(&ship.sof_hull)
        .bind(&ship.sof_faction)
        .bind(&ship.sof_race)
        .bind(&ship.colors.primary)
        .bind(&ship.colors.secondary)
        .bind(&ship.colors.hull)
        .bind(
            ship.faction_id
                .filter(|id| tables.factions.iter().any(|faction| faction.id == *id)),
        )
        .execute(&mut *tx)
        .await?;
    }

    for skin in &tables.skins {
        sqlx::query(
            "insert into ship_skins (ship_id, skin_id, material_id, name, sof_faction,
                color_primary, color_secondary, color_hull, dna_suffix)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(skin.ship_id)
        .bind(skin.skin_id)
        .bind(skin.material_id)
        .bind(&skin.name)
        .bind(&skin.sof_faction)
        .bind(&skin.colors.primary)
        .bind(&skin.colors.secondary)
        .bind(&skin.colors.hull)
        .bind(&skin.dna_suffix)
        .execute(&mut *tx)
        .await?;
    }

    for attribute in &tables.attributes {
        sqlx::query("insert into ship_attributes (ship_id, name, value) values ($1, $2, $3)")
            .bind(attribute.ship_id)
            .bind(&attribute.name)
            .bind(attribute.value)
            .execute(&mut *tx)
            .await?;
    }

    for bonus in &tables.bonuses {
        sqlx::query(
            "insert into ship_bonuses (ship_id, position, kind, skill_id, skill_name, bonus, unit, text)
             values ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(bonus.ship_id)
        .bind(bonus.position)
        .bind(bonus.kind.as_str())
        .bind(bonus.skill_id)
        .bind(&bonus.skill_name)
        .bind(bonus.bonus)
        .bind(&bonus.unit)
        .bind(&bonus.text)
        .execute(&mut *tx)
        .await?;
    }

    for component in &tables.skinr_components {
        sqlx::query(
            "insert into skinr_components (id, name, category, finish, rarity, resource_file,
                icon_file, projection_u, projection_v)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(component.id)
        .bind(&component.name)
        .bind(&component.category)
        .bind(&component.finish)
        .bind(component.rarity)
        .bind(&component.resource_file)
        .bind(&component.icon_file)
        .bind(&component.projection_u)
        .bind(&component.projection_v)
        .execute(&mut *tx)
        .await?;
    }

    for slot in &tables.skinr_slot_materials {
        sqlx::query(
            "insert into skinr_slot_materials (faction_id, slot_id, material_id) values ($1, $2, $3)",
        )
        .bind(slot.faction_id)
        .bind(slot.slot_id)
        .bind(slot.material_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await
}

/// Stores the extracted faction logos, replacing earlier copies by name.
pub async fn store_logos(pool: &PgPool, logos: &[super::logos::Logo]) -> sqlx::Result<()> {
    for logo in logos {
        sqlx::query(
            "insert into faction_logos (name, png, width, height) values ($1, $2, $3, $4)
             on conflict (name) do update
             set png = excluded.png, width = excluded.width, height = excluded.height",
        )
        .bind(&logo.name)
        .bind(&logo.png)
        .bind(logo.width as i32)
        .bind(logo.height as i32)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Stores attribute icons (name and PNG bytes), replacing earlier copies.
pub async fn store_skin_icon_layers(pool: &PgPool, layers: &[(&str, Vec<u8>)]) -> sqlx::Result<()> {
    for (name, png) in layers {
        sqlx::query(
            "insert into skin_icon_layers (name, png) values ($1, $2)
             on conflict (name) do update set png = excluded.png",
        )
        .bind(name)
        .bind(png)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn store_skinr_component_icons(
    pool: &PgPool,
    icons: &[(i32, Vec<u8>)],
) -> sqlx::Result<()> {
    for (component_id, png) in icons {
        sqlx::query(
            "insert into skinr_component_icons (component_id, png) values ($1, $2)
             on conflict (component_id) do update set png = excluded.png",
        )
        .bind(component_id)
        .bind(png)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn store_skin_icon_colors(pool: &PgPool, colors: &[(i32, String)]) -> sqlx::Result<()> {
    for (material_id, colors) in colors {
        sqlx::query("update skin_icons set colors = $2 where material_id = $1")
            .bind(material_id)
            .bind(colors)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn store_skinr_component_colors(
    pool: &PgPool,
    colors: &[(i32, String)],
) -> sqlx::Result<()> {
    for (component_id, color) in colors {
        sqlx::query("update skinr_components set color = $2 where id = $1")
            .bind(component_id)
            .bind(color)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn store_skin_icons(pool: &PgPool, icons: &[(i32, Vec<u8>)]) -> sqlx::Result<()> {
    for (material_id, png) in icons {
        sqlx::query(
            "insert into skin_icons (material_id, png) values ($1, $2)
             on conflict (material_id) do update set png = excluded.png",
        )
        .bind(material_id)
        .bind(png)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn store_attribute_icons(pool: &PgPool, icons: &[(String, Vec<u8>)]) -> sqlx::Result<()> {
    for (name, png) in icons {
        sqlx::query(
            "insert into attribute_icons (name, png) values ($1, $2)
             on conflict (name) do update set png = excluded.png",
        )
        .bind(name)
        .bind(png)
        .execute(pool)
        .await?;
    }
    Ok(())
}
