import type { SkinrDesign, ShipSkinrs } from './types';

// EveOffline/scripts/DataSync/lib/everefSkinr.js is the target converter.
const BLEND_MODES: Record<string, number> = {
  normal: 1,
  subtract: 2,
  exclusion: 3,
  nested: 4,
  nested_inverted: 5,
};
const PROJECTION_TYPES: Record<string, number> = {
  // Native SKINR wire values differ from the SOF renderer's enum.
  'clamp-to-border': 0,
  repeat: 1,
  'clamp-to-edge': 2,
};
const MAX_EXPORT_BYTES = 64 * 1024;
const MAX_NAME_LENGTH = 20;
const MAX_LINE_LENGTH = 50;

export interface EvejsTarget {
  format: 'eveoffline.skinr-export-target';
  version: 1;
  generated_at: string;
  target: string;
  ships: Record<string, number[]>;
  slots: Record<string, number[]>;
  components: Record<string, { category: number; available: boolean; disallowed_slots: number[] }>;
}

/** Native EveOffline portable draft recipe. Ownership, retail hashes and licenses are omitted. */
export function evejsExport(
  ship: { id: number; name: string; dna: string },
  design: SkinrDesign,
  skinrs: ShipSkinrs,
  target?: EvejsTarget | null,
) {
  if (target && !target.ships[ship.id])
    throw new Error('This hull is not supported by the installed EveOffline SKINR data.');
  const line = design.line ?? '';
  if (
    !design.name.trim() ||
    [...design.name].length > MAX_NAME_LENGTH ||
    [...line].length > MAX_LINE_LENGTH ||
    /[\p{Cc}\p{Cs}]/u.test(design.name + line)
  ) {
    throw new Error(
      'EveOffline requires a name of 1–20 characters and a line name of at most 50 characters, without control characters.',
    );
  }
  const blend = BLEND_MODES[design.layout.pattern_blend_mode ?? 'normal'];
  if (!blend) throw new Error('This pattern blend mode is not supported by EveOffline.');
  const fitted = new Set(design.layout.slots.map((slot) => slot.id));
  if (!fitted.size || fitted.size > 8 || fitted.size !== design.layout.slots.length) {
    throw new Error('EveOffline requires between one and eight unique fitted slots.');
  }
  if (
    (fitted.has(7) && !fitted.has(5)) ||
    (fitted.has(6) && !fitted.has(5)) ||
    (fitted.has(8) && !fitted.has(7))
  ) {
    throw new Error('EveOffline cannot import an unused pattern or pattern material.');
  }
  if (
    [2, 3].includes(blend)
      ? fitted.has(8)
      : fitted.has(5) !== fitted.has(6) || fitted.has(7) !== fitted.has(8)
  ) {
    throw new Error('The pattern material slots do not match EveOffline’s blend-mode rules.');
  }
  const slots = design.layout.slots
    .map((slot) => {
      if (target) {
        const id = slot.configuration.nanocoating?.id ?? slot.configuration.pattern?.id;
        const component = id === undefined ? null : target.components[id];
        const isPattern = Boolean(slot.configuration.pattern);
        if (
          !target.ships[ship.id]?.includes(slot.id) ||
          !component?.available ||
          component.disallowed_slots.includes(slot.id) ||
          !target.slots[slot.id]?.includes(component.category) ||
          (component.category === 2) !== isPattern
        ) {
          throw new Error(
            `EveOffline’s installed data does not allow component ${id ?? 'unknown'} in slot ${slot.id}.`,
          );
        }
      }
      const coating = slot.configuration.nanocoating;
      if (coating) {
        if (!skinrs.components[coating.id])
          throw new Error(
            `Component ${coating.id} is unavailable; refresh the ship catalogue before exporting.`,
          );
        // ESI has no coating overrides. Zero vectors select native component defaults.
        const configuration = Object.fromEntries(
          ['diffuse_color', 'dust_diffuse_color', 'fresnel_color', 'gloss'].map((field) => [
            field,
            { x: 0, y: 0, z: 0, w: 0 },
          ]),
        );
        return {
          id: { sequential: slot.id },
          configuration: {
            coating: { coating: { sequential: coating.id }, configuration },
          },
        };
      }
      const pattern = slot.configuration.pattern;
      if (!pattern) throw new Error(`Slot ${slot.id} has no component to export.`);
      const component = skinrs.components[pattern.id];
      const typeU = PROJECTION_TYPES[component?.projection_u ?? ''];
      const typeV = PROJECTION_TYPES[component?.projection_v ?? ''];
      if (typeU === undefined || typeV === undefined)
        throw new Error(`Pattern projection data for component ${pattern.id} is unavailable.`);
      const config = pattern.configuration;
      if (!config.transform || typeof config.mirrored !== 'boolean')
        throw new Error(
          `Pattern ${pattern.id} has an incomplete transform or mirroring configuration.`,
        );
      const vectors = [
        config.transform.position,
        config.transform.rotation,
        config.transform.scaling,
      ];
      if (
        vectors.some((vector) =>
          Object.values(vector).some(
            (value) => !Number.isFinite(value) || Math.abs(value) > 1_000_000,
          ),
        )
      ) {
        throw new Error('Pattern transforms exceed EveOffline’s supported range.');
      }
      const scales = Object.values(config.transform.scaling);
      if (
        scales.some((value) => value <= 0) ||
        Math.max(...scales) - Math.min(...scales) > 0.000001
      ) {
        throw new Error('EveOffline requires positive, uniform pattern scaling.');
      }
      const projection: Record<string, boolean | number> = {
        projection_type_u: typeU,
        projection_type_v: typeV,
      };
      for (const index of [1, 2, 3, 4] as const) {
        const enabled = config.projection?.[`slot${index}`];
        if (typeof enabled !== 'boolean')
          throw new Error(`Pattern ${pattern.id} has incomplete material projection data.`);
        projection[`area${index}`] = enabled;
      }
      return {
        id: { sequential: slot.id },
        configuration: {
          pattern: {
            pattern: { sequential: pattern.id },
            configuration: {
              projection,
              transform: structuredClone(config.transform),
              mirrored: config.mirrored,
            },
          },
        },
      };
    })
    .sort((a, b) => a.id.sequential - b.id.sequential);
  const document = {
    format: 'eveoffline.skinr-design' as const,
    version: 1 as const,
    design: {
      name: design.name,
      line_name: line,
      layout: { ship_type: { sequential: ship.id }, slots, pattern_blend_mode: blend },
      editor_configurations: [],
    },
  };
  if (
    new TextEncoder().encode(JSON.stringify(document, null, 2) + '\n').length > MAX_EXPORT_BYTES
  ) {
    throw new Error('This design exceeds EveOffline’s 64 KiB import limit.');
  }
  return document;
}

export function evejsFilename(name: string, id: string): string {
  const safe = name
    .normalize('NFKD')
    .replace(/[^a-z0-9_-]+/gi, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40);
  return `${safe || 'skinr'}-${id}.skinr.json`;
}
