//! Native import of the ship reference data from CCP's JSONL Static Data
//! Export: [`client`] downloads it, [`parse`] turns it into [`ShipTables`],
//! [`seed`] writes those into Postgres.

pub mod client;
pub mod dogma;
pub mod logos;
pub mod parse;
pub mod seed;
pub mod skin_icon_colors;
pub mod skinr;

/// Everything the viewer stores from one SDE build.
#[derive(Debug, Default)]
pub struct ShipTables {
    pub races: Vec<Race>,
    pub factions: Vec<Faction>,
    pub meta_groups: Vec<MetaGroup>,
    pub groups: Vec<ShipGroup>,
    pub ships: Vec<Ship>,
    pub skins: Vec<ShipSkin>,
    pub attributes: Vec<ShipAttribute>,
    pub bonuses: Vec<ShipBonus>,
    /// The client icon of each sheet attribute: attribute name and `res:/` path.
    pub attribute_icons: Vec<dogma::AttributeIcon>,
    pub skinr_components: Vec<skinr::SkinrComponent>,
    pub skinr_slot_materials: Vec<skinr::SlotMaterial>,
}

/// One dogma attribute value of a ship, by attribute name.
#[derive(Debug, Clone, PartialEq)]
pub struct ShipAttribute {
    pub ship_id: i32,
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BonusKind {
    /// Per level of a skill.
    Skill,
    /// Fixed role bonus.
    Role,
    /// Anything else the SDE lists without a bonus figure.
    Misc,
}

impl BonusKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BonusKind::Skill => "skill",
            BonusKind::Role => "role",
            BonusKind::Misc => "misc",
        }
    }
}

/// One line of a ship's bonuses: "5% bonus to ..." under its skill or role.
#[derive(Debug, Clone, PartialEq)]
pub struct ShipBonus {
    pub ship_id: i32,
    pub position: i32,
    pub kind: BonusKind,
    pub skill_id: Option<i32>,
    pub skill_name: Option<String>,
    pub bonus: Option<f64>,
    pub unit: Option<String>,
    /// EVE HTML.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Race {
    pub id: i32,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Faction {
    pub id: i32,
    pub name: String,
    /// The SDE's `flatLogo`, naming the emblem in the game client.
    pub logo: Option<String>,
    /// EVE HTML, as the SDE has it.
    pub description: String,
    /// The SDE's `shortDescription`, a one-line motto; many factions have none.
    pub tagline: Option<String>,
    /// The race this faction is the home empire of: one with a militia of
    /// its own and a single member race (Amarr, Caldari, Gallente, Minmatar).
    pub empire_of: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetaGroup {
    pub id: i32,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipGroup {
    pub id: i32,
    pub name: String,
}

/// A livery's swatch colors as `#rrggbb`, from its SDE material set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveryColors {
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub hull: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ship {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub group_id: i32,
    pub race_id: Option<i32>,
    pub faction_id: Option<i32>,
    pub meta_group_id: Option<i32>,
    pub tech_level: Option<i32>,
    pub mass: Option<f64>,
    pub volume: Option<f64>,
    pub capacity: Option<f64>,
    pub radius: Option<f64>,
    pub sof_hull: String,
    pub sof_faction: String,
    pub sof_race: String,
    /// The default livery's colors.
    pub colors: LiveryColors,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipSkin {
    pub ship_id: i32,
    pub skin_id: i32,
    /// The skin material, which names the client's icon for it.
    pub material_id: i32,
    pub name: String,
    pub sof_faction: String,
    /// Material, pattern and texture overrides from the graphic material set.
    pub dna_suffix: String,
    pub colors: LiveryColors,
}
