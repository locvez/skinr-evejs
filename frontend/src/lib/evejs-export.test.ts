import { describe, expect, it } from 'vitest';
import { evejsExport, evejsFilename, type EvejsTarget } from './evejs-export';
import { skinrLook } from './skinr';
import type { SkinrDesign, ShipSkinrs } from './types';

const design: SkinrDesign = {
  id: 'abcdef0123456789',
  name: 'Archived design',
  line: 'My line',
  tier: 5,
  creator: { id: 42, name: null },
  listings: 0,
  quantity: 0,
  min_price_plex: null,
  layout: {
    slots: [{ id: 1, configuration: { nanocoating: { id: 168 } } }],
    pattern_blend_mode: 'normal',
  },
};
const skinrs: ShipSkinrs = {
  designs: [design],
  slot_materials: { 1: 3, 2: 2, 3: 4, 4: 1 },
  components: {
    168: {
      id: 168,
      name: 'Black',
      category: 'Material',
      finish: 'Matte',
      material: 'black_satin_matt',
      texture: null,
      projection_u: null,
      projection_v: null,
      icon_url: null,
      color: '#111111',
    },
  },
};

describe('EveJS JSON transfer', () => {
  it('writes EveOffline native identifiers and omits ownership and market state', () => {
    const bundle = evejsExport(
      { id: 601, name: 'Ibis', dna: 'cf5_t1:caldaribase:caldari' },
      design,
      skinrs,
    );
    const roundtrip = JSON.parse(JSON.stringify(bundle));
    expect(roundtrip.format).toBe('eveoffline.skinr-design');
    expect(roundtrip.version).toBe(1);
    expect(roundtrip.design).toMatchObject({
      name: design.name,
      line_name: 'My line',
      layout: {
        ship_type: { sequential: 601 },
        pattern_blend_mode: 1,
        slots: [
          {
            id: { sequential: 1 },
            configuration: {
              coating: {
                coating: { sequential: 168 },
                configuration: { diffuse_color: { x: 0, y: 0, z: 0, w: 0 } },
              },
            },
          },
        ],
      },
      editor_configurations: [],
    });
    expect(Object.keys(roundtrip.design).sort()).toEqual([
      'editor_configurations',
      'layout',
      'line_name',
      'name',
    ]);
  });

  it('refuses an export with missing component data', () => {
    expect(() =>
      evejsExport({ id: 601, name: 'Ibis', dna: 'cf5_t1:caldaribase:caldari' }, design, {
        ...skinrs,
        components: {},
      }),
    ).toThrow('Component 168');
  });

  it('makes a safe filename from player-authored names', () => {
    expect(evejsFilename('../My "Skin"\n', design.id)).toBe('My-Skin-abcdef0123456789.skinr.json');
  });

  it('preserves pattern transforms, mirroring and native projection enums', () => {
    const transform = {
      position: { x: 10, y: -20, z: 30 },
      rotation: { x: 0, y: 0, z: 0, w: 1 },
      scaling: { x: 5, y: 5, z: 5 },
    };
    const patterned = {
      ...design,
      layout: {
        pattern_blend_mode: 'subtract',
        slots: [
          {
            id: 5,
            configuration: {
              pattern: {
                id: 123,
                configuration: {
                  mirrored: true,
                  transform,
                  projection: { slot1: true, slot2: false, slot3: true, slot4: false },
                },
              },
            },
          },
        ],
      },
    };
    const bundle = evejsExport({ id: 601, name: 'Ibis', dna: '' }, patterned, {
      ...skinrs,
      components: {
        123: {
          ...skinrs.components[168],
          id: 123,
          category: 'Pattern',
          material: null,
          texture: 'res:/pattern.dds',
          projection_u: 'clamp-to-border',
          projection_v: 'repeat',
        },
      },
    });
    expect(bundle.design.layout.pattern_blend_mode).toBe(2);
    expect(bundle.design.layout.slots[0].configuration.pattern?.configuration).toEqual({
      transform,
      mirrored: true,
      projection: {
        area1: true,
        area2: false,
        area3: true,
        area4: false,
        projection_type_u: 0,
        projection_type_v: 1,
      },
    });
    expect(
      skinrLook(
        'cf5_t1:caldaribase:caldari',
        patterned,
        {
          123: {
            ...skinrs.components[168],
            id: 123,
            category: 'Pattern',
            material: null,
            texture: 'res:/pattern.dds',
            projection_u: 'clamp-to-border',
            projection_v: 'repeat',
          },
        },
        skinrs.slot_materials,
      ).pattern?.record,
    ).toMatchObject({
      layer1: { projectionTypeU: 2, projectionTypeV: 0 },
    });
  });

  it('rejects names that EveOffline cannot import', () => {
    expect(() =>
      evejsExport({ id: 601, name: 'Ibis', dna: '' }, { ...design, name: 'x'.repeat(21) }, skinrs),
    ).toThrow('1–20');
  });

  it('refuses unsupported hulls and unavailable target components before download', () => {
    const ship = { id: 601, name: 'Ibis', dna: '' };
    const target: EvejsTarget = {
      format: 'eveoffline.skinr-export-target',
      version: 1,
      generated_at: '',
      target: 'EveOffline',
      ships: { 601: [1] },
      slots: { 1: [1, 3] },
      components: { 168: { category: 1, available: true, disallowed_slots: [] } },
    };
    expect(evejsExport(ship, design, skinrs, target).format).toBe('eveoffline.skinr-design');
    expect(() => evejsExport(ship, design, skinrs, { ...target, ships: {} })).toThrow('hull');
    expect(() => evejsExport(ship, design, skinrs, { ...target, components: {} })).toThrow(
      'component 168',
    );
    expect(() =>
      evejsExport(ship, design, skinrs, {
        ...target,
        components: {
          168: {
            category: 1,
            available: true,
            disallowed_slots: [1],
          },
        },
      }),
    ).toThrow('slot 1');
  });
});
