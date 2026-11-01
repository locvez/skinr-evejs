// Validates every audited appearance against the live client SOF catalogue, without a GPU.
// Run scripts/audit-skins.ts first, then this script. Results go to storage/skin-dna-audit.json.
// Register synthetic patterns before the bounded validation queue starts.
/* eslint-disable no-await-in-loop */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { CjsFileIndexLibrary } from '@carbonenginejs/runtime/tools/fileindex';
import { skinrLook } from '../src/lib/skinr.ts';
import { openSof } from '../src/lib/viewer/resources.ts';

const BASE = process.env.BASE_URL ?? 'http://localhost:5200';
const STORAGE = new URL('../../storage/', import.meta.url);
const CACHE = new URL('audit-cdn/', STORAGE);
const WORKERS = 16;
const PROGRESS_INTERVAL = 1000;
await mkdir(CACHE, { recursive: true });
const payloads = JSON.parse(await readFile(new URL('catalog-api.json', STORAGE), 'utf8'));
const library = await CjsFileIndexLibrary.load(
  {
    game: 'Eve',
    id: 'ccp',
    defaultBuildRef: 'latest',
    remote: {
      metadataBaseURL: `${BASE}/cdn/binaries`,
      indexBaseURL: `${BASE}/cdn/binaries`,
      appBaseURL: `${BASE}/cdn/binaries`,
      resBaseURL: `${BASE}/cdn/resources`,
    },
    clients: { tranquility: { metadataToken: 'TQ', aliases: ['tq'] } },
  },
  { build: 'latest' },
);
const resolve = (path: string) =>
  library.Resolve(path) ??
  (path.endsWith('.red') ? library.Resolve(`${path.slice(0, -4)}.black`) : null);
const sof = await openSof(library, async (path: string) => {
  const hit = resolve(path);
  if (!hit) throw new Error(`Not in client index: ${path}`);
  const cacheFile = new URL(createHash('sha256').update(hit.sourceURL).digest('hex'), CACHE);
  try {
    return await readFile(cacheFile);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
  }
  const response = await fetch(hit.sourceURL, { signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`HTTP ${response.status} for ${path}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  await writeFile(cacheFile, bytes);
  return bytes;
});
const failures: any[] = [];
const counts = { base: 0, ccp: 0, skinr: 0 };
const queue: any[] = [];
for (const { ship, skinrs } of payloads) {
  const looks = [
    { kind: 'base', name: 'Standard issue', dna: ship.dna },
    ...ship.skins.map((skin: any) => ({ kind: 'ccp', ...skin })),
    ...skinrs.designs.map((design: any) => {
      const look = skinrLook(ship.dna, design, skinrs.components, skinrs.slot_materials);
      if (look.pattern) sof.dataMgr.UpdatePattern(look.pattern.name, look.pattern.record);
      return { kind: 'skinr', id: design.id, name: design.name, dna: look.dna };
    }),
  ];
  queue.push(...looks.map((look) => Object.assign({ ship: ship.name }, look)));
}
let next = 0;
let completed = 0;
await Promise.all(
  Array.from({ length: WORKERS }, async () => {
    while (next < queue.length) {
      const look = queue[next++];
      counts[look.kind as keyof typeof counts]++;
      try {
        const inspected = await sof.InspectDna(look.dna);
        if (!inspected.valid) failures.push({ ...look, ...inspected });
      } catch (error) {
        failures.push({ ...look, error: String(error) });
      }
      completed++;
      if (completed % PROGRESS_INTERVAL === 0)
        console.log(`Validated ${completed}/${queue.length} appearances`);
    }
  }),
);
const report = { checkedAt: new Date().toISOString(), build: library.build, counts, failures };
await writeFile(new URL('skin-dna-audit.json', STORAGE), JSON.stringify(report, null, 2));
console.log(
  JSON.stringify(
    {
      ...report,
      failures: failures.map(({ ship, kind, name, error }) => ({ ship, kind, name, error })),
    },
    null,
    2,
  ),
);
if (failures.length) process.exitCode = 1;
