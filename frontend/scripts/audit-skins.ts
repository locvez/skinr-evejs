// Read-only audit of a running catalogue against the cached SDE and a database snapshot.
// Prepare inputs with frontend/scripts/snapshot-skin-catalog.ps1 from the repository root.
// Results go to storage/catalog-audit.json; no production fixtures are written.
// Workers walk their queues sequentially to bound requests; _key is CCP's field name.
/* eslint-disable no-await-in-loop, no-underscore-dangle */
import { readFile, writeFile } from 'node:fs/promises';
import { skinrLook } from '../src/lib/skinr.ts';

const BASE = process.env.BASE_URL ?? 'http://localhost:5200';
const STORAGE = new URL('../../storage/', import.meta.url);
const WORKERS = 6;
const input = async (name: string) =>
  JSON.parse((await readFile(new URL(name, STORAGE), 'utf8')).replace(/^\uFEFF/, ''));
const jsonl = async (name: string) =>
  (await readFile(new URL(`audit-sde/${name}.jsonl`, STORAGE), 'utf8'))
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line));
const db = await input('catalog-db.json');
const sourceSkins = await jsonl('skins');
const materials = new Map((await jsonl('skinMaterials')).map((row) => [row._key, row]));
const sets = new Map((await jsonl('graphicMaterialSets')).map((row) => [row._key, row]));
const sourceByShip = new Map<number, any[]>();
for (const skin of sourceSkins.filter((candidate) => candidate.visibleTranquility)) {
  for (const ship of skin.types ?? []) {
    const rows = sourceByShip.get(ship) ?? [];
    rows.push(skin);
    sourceByShip.set(ship, rows);
  }
}
const get = async (path: string) => {
  const response = await fetch(`${BASE}${path}`);
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  return response;
};
const failures: any[] = [];
if (db.missing_active_designs)
  failures.push({
    ship: 'Catalogue',
    message: 'Active SKINR listings lack design details',
    details: db.missing_active_designs,
  });
if (db.unknown_ship_designs)
  failures.push({
    ship: 'Catalogue',
    message: 'Active SKINR designs reference absent ships',
    details: db.unknown_ship_designs,
  });
const results: any[] = [];
const payloads: any[] = [];
let next = 0;
await Promise.all(
  Array.from({ length: WORKERS }, async () => {
    while (next < db.ships.length) {
      const summary = db.ships[next++];
      const fail = (message: string, details?: unknown) =>
        failures.push({ ship: summary.name, id: summary.id, message, details });
      try {
        const [ship, skinrs, html] = await Promise.all([
          get(`/api/ships/${summary.id}`).then((r) => r.json()),
          get(`/api/ships/${summary.id}/skinrs`).then((r) => r.json()),
          get(`/ships/${summary.id}`).then((r) => r.text()),
        ]);
        const raw = sourceByShip.get(summary.id) ?? [];
        const seen = new Set<string>();
        const expected: number[] = [];
        const excluded: any[] = [];
        // Compare the full authored appearance, independently of the Rust parser.
        for (const source of [...raw].sort((a, b) => a._key - b._key)) {
          const material = materials.get(source.skinMaterialID);
          const set = sets.get(material?.materialSetID);
          if (!set) {
            fail('SDE skin has no material set', source._key);
            continue;
          }
          const faction = set.sofFactionName ?? summary.sof_faction;
          const overrides = [
            'material1',
            'material2',
            'material3',
            'material4',
            'sofPatternName',
            'resPathInsert',
          ].some((key) => set[key] != null);
          const signature = JSON.stringify([
            faction,
            ...[
              'material1',
              'material2',
              'material3',
              'material4',
              'sofPatternName',
              'custommaterial1',
              'custommaterial2',
              'resPathInsert',
            ].map((key) => set[key] ?? null),
          ]);
          if (faction === summary.sof_faction && !overrides) {
            excluded.push({ id: source._key, reason: 'base appearance' });
          } else if (seen.has(signature)) {
            excluded.push({ id: source._key, reason: 'duplicate appearance' });
          } else {
            seen.add(signature);
            expected.push(source._key);
          }
        }
        const actual = ship.skins.map((skin: any) => skin.id).sort((a: number, b: number) => a - b);
        if (JSON.stringify(actual) !== JSON.stringify(expected.sort((a, b) => a - b)))
          fail('CCP skin IDs differ from SDE', { expected, actual });
        const expectedDesigns = db.designs.filter((d: any) => d.ship_type_id === summary.id);
        const expectedIds = expectedDesigns.map((d: any) => d.id).sort();
        const actualIds = skinrs.designs.map((d: any) => d.id).sort();
        if (JSON.stringify(expectedIds) !== JSON.stringify(actualIds))
          fail('SKINR IDs differ from listed or imported designs', { expectedIds, actualIds });
        for (const design of skinrs.designs) {
          const expectedDesign = expectedDesigns.find((d: any) => d.id === design.id);
          if (
            expectedDesign &&
            (expectedDesign.listings !== design.listings ||
              expectedDesign.quantity !== design.quantity ||
              expectedDesign.min_price_plex !== design.min_price_plex)
          )
            fail('Listing aggregate mismatch', design.id);
          for (const slot of design.layout.slots) {
            const id = slot.configuration.nanocoating?.id ?? slot.configuration.pattern?.id;
            if (id !== undefined && !skinrs.components[id])
              fail('Missing SKINR component', { design: design.id, component: id });
          }
          if (Object.keys(skinrs.slot_materials).length !== 4)
            fail('Missing hull material mapping', design.id);
          skinrLook(ship.dna, design, skinrs.components, skinrs.slot_materials);
        }
        if (!html.includes('id="liveries"')) fail('Page hides skin section');
        const railCount = (label: string) =>
          Number(
            html.match(
              new RegExp(`>${label}</span> <span class="ml-1.5 tabular-nums">(\\d+)</span>`),
            )?.[1] ?? 0,
          );
        if (railCount('In-game') !== ship.skins.length + 1)
          fail('Rendered CCP rail count mismatch', railCount('In-game'));
        if (railCount('SKINR') !== skinrs.designs.length)
          fail('Rendered SKINR rail count mismatch', railCount('SKINR'));
        if (html.includes('SKINR designs couldn’t be loaded')) fail('Page failed to load SKINR');
        results.push({
          id: summary.id,
          name: summary.name,
          ccp: ship.skins.length,
          skinr: skinrs.designs.length,
          source: raw.length,
          excluded,
        });
        payloads.push({ ship, skinrs });
      } catch (error) {
        fail(String(error));
      }
    }
  }),
);
results.sort((a, b) => a.id - b.id);
const report = {
  checkedAt: new Date().toISOString(),
  inputs: [
    'storage/catalog-db.json (read-only Postgres snapshot)',
    'storage/audit-sde/*.jsonl (cached official CCP SDE)',
  ],
  ships: results.length,
  ccpSkins: results.reduce((total, ship) => total + ship.ccp, 0),
  skinrDesigns: results.reduce((total, ship) => total + ship.skinr, 0),
  failures,
  results,
};
await writeFile(new URL('catalog-audit.json', STORAGE), JSON.stringify(report, null, 2));
await writeFile(new URL('catalog-api.json', STORAGE), JSON.stringify(payloads));
console.log(
  JSON.stringify(
    {
      ...report,
      failures: failures.map(({ ship, message }) => ({ ship, message })),
      results: undefined,
    },
    null,
    2,
  ),
);
if (failures.length) process.exitCode = 1;
