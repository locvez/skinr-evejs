// The JSON shapes of the Axum API (src/server/api.rs).

export interface Named {
  id: number;
  name: string;
}

export interface MetaGroup extends Named {
  color: string | null;
}

/** A livery's swatch colors as #rrggbb, from its SDE material set. */
export interface Colors {
  primary: string | null;
  secondary: string | null;
  hull: string | null;
}

/** A faction and its emblem, extracted from the game client by the import. */
export interface FactionInfo {
  id: number;
  name: string;
  logo_url: string | null;
  /** The race this faction is the home empire of (Amarr, Caldari, Gallente, Minmatar). */
  empire_of: number | null;
}

/** A ship's faction in full, for the blueprint's faction panel. */
export interface FactionDetail extends FactionInfo {
  /** A one-line motto; many factions have none. */
  tagline: string | null;
  description: string;
}

export interface ShipSummary {
  id: number;
  name: string;
  group_id: number;
  race_id: number | null;
  faction_id: number | null;
  meta_group_id: number | null;
  tech_level: number | null;
  dna: string;
  colors: Colors;
}

export interface Catalog {
  sde_build: string | null;
  races: Named[];
  factions: FactionInfo[];
  groups: Named[];
  meta_groups: MetaGroup[];
  ships: ShipSummary[];
}

export interface Skin {
  id: number;
  name: string;
  dna: string;
  colors: Colors;
  /** The game's icon for the skin material, when the client has one. */
  icon_url: string | null;
  /** That icon's colors, read off it by the import, to draw it sharp as vectors. */
  icon_colors: IconColors | null;
}

/** A skin icon's colors: its three bands and its lights. */
export interface IconColors extends Colors {
  lights: string;
}

/**
 * A run of EVE HTML text with its styles, or a line break (see the Rust
 * eve_html module). Rendered as text, never as markup.
 */
export type RichSegment =
  | {
      text: string;
      bold?: boolean;
      italic?: boolean;
      underline?: boolean;
      /** A validated CSS color, #rrggbbaa. */
      color?: string;
      /** A ship this text links to. */
      type_id?: number;
    }
  | { br: true };

export interface ShipDetail {
  id: number;
  name: string;
  /** The description as plain text. */
  description: string;
  description_rich: RichSegment[];
  group: Named;
  race: Named | null;
  faction: FactionDetail | null;
  meta_group: MetaGroup | null;
  tech_level: number | null;
  mass: number | null;
  volume: number | null;
  capacity: number | null;
  radius: number | null;
  dna: string;
  colors: Colors;
  skins: Skin[];
  sheet: Sheet;
  bonuses: BonusGroup[];
}

/** A ship's spec sheet (src/sheet.rs): figures as the game's Show Info prints them. */
export interface Sheet {
  slots: { high: number; mid: number; low: number; rig: number; turret: number; launcher: number };
  fitting: {
    powergrid: number | null;
    cpu: number | null;
    calibration: number | null;
    drone_bay: number | null;
    drone_bandwidth: number | null;
    fighter_bay: number | null;
    fighter_tubes: number | null;
  };
  /** Shield, armor, structure; resistances in percent. */
  defense: {
    name: string;
    hp: number;
    em: number;
    thermal: number;
    kinetic: number;
    explosive: number;
    recharge?: number;
  }[];
  capacitor: { capacity: number | null; recharge: number | null };
  targeting: {
    range: number | null;
    max_targets: number | null;
    scan_resolution: number | null;
    signature_radius: number | null;
    sensor: { kind: string; strength: number } | null;
  };
  navigation: {
    max_velocity: number | null;
    warp_speed: number | null;
    align_time: number | null;
    inertia: number | null;
  };
}

export interface BonusGroup {
  title: string;
  per_level: boolean;
  lines: { amount: string | null; text: RichSegment[] }[];
}

/** A SKINR component (src/server/skinr.rs): a material or a projected pattern. */
export interface SkinrComponent {
  id: number;
  name: string;
  /** `Material`, `Pattern` or `Metallic`. */
  category: string;
  finish: string | null;
  /** For materials: the SOF material name DNA uses. */
  material: string | null;
  /** For patterns: the projection texture. */
  texture: string | null;
  projection_u: string | null;
  projection_v: string | null;
  icon_url: string | null;
  /** For materials: a swatch color sampled from the client's icon, `#rrggbb`. */
  color: string | null;
}

export interface SkinrVector {
  x: number;
  y: number;
  z: number;
}

/** What one SKINR slot holds, exactly as ESI returns it. */
export interface SkinrSlotConfiguration {
  nanocoating?: { id: number };
  pattern?: {
    id: number;
    configuration: {
      /** Which of the four SKINR material slots the pattern is projected onto. */
      projection?: Partial<Record<'slot1' | 'slot2' | 'slot3' | 'slot4', boolean>>;
      transform?: {
        position: SkinrVector;
        rotation: SkinrVector & { w: number };
        scaling: SkinrVector;
      };
      mirrored?: boolean;
    };
  };
}

/** A player-made design listed on the Paragon Hub. */
export interface SkinrDesign {
  id: string;
  name: string;
  line: string | null;
  tier: number | null;
  creator: { id: number; name: string | null };
  layout: {
    slots: { id: number; configuration: SkinrSlotConfiguration }[];
    pattern_blend_mode?: string;
  };
  listings: number;
  quantity: number;
  min_price_plex: number | null;
  imported?: boolean;
  source_file?: string | null;
}

export interface SkinrImportStatus {
  folder_available: boolean;
  files: { name: string; bytes: number; kind: 'designs' | 'listings' }[];
  imported_designs: number;
  ships: { id: number; name: string; designs: number }[];
}

export interface SkinrImportReport {
  files: number;
  designs: number;
  added: number;
  already_imported: number;
}

/** `/api/ships/{id}/skinrs`. */
export interface ShipSkinrs {
  /** SKINR material slot (1-4) to Trinity hull material (1-4), for this hull's faction. */
  slot_materials: Record<string, number>;
  designs: SkinrDesign[];
  components: Record<string, SkinrComponent>;
}
