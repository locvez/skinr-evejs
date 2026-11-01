// GPU smoke checks across factions, hull sizes and both skin sources.
// Keep one page at a time to bound GPU memory and attribute errors to one hull.
/* eslint-disable no-await-in-loop */
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const BASE = process.env.BASE_URL ?? 'http://localhost:5200';
const STORAGE = new URL('../../storage/', import.meta.url);
const samples = process.argv.slice(2);
const repairs = samples.includes('--repairs');
const names = samples.length
  ? samples
  : [
      'Merlin',
      'Punisher',
      'Tristan',
      'Rifter',
      'Gila',
      'Bhaalgorn',
      'Nightmare',
      'Machariel',
      'Vindicator',
      'Astero',
      'Kikimora',
      'Vedmak',
      'Stormbringer',
      'Orca',
      'Hulk',
      'Rorqual',
      'Revelation',
      'Avatar',
      'Ishtar',
      'Paladin',
      'Loki',
      'Gnosis',
      'Sunesis',
      'Zephyr',
    ];
const payloads = JSON.parse(await readFile(new URL('catalog-api.json', STORAGE), 'utf8'));
const repairData = repairs
  ? JSON.parse(await readFile(new URL('skin-dna-audit-before-repairs.json', STORAGE), 'utf8'))
  : null;
const cases = repairs
  ? [
      ...new Map(
        repairData.failures.map((failure) => [
          failure.name,
          { name: failure.ship, skinId: failure.id },
        ]),
      ).values(),
    ]
  : names.map((name) => ({ name, skinId: null }));
const browser = await chromium.launch({
  channel: process.env.BROWSER_CHANNEL ?? 'msedge',
  headless: true,
  args: ['--enable-unsafe-webgpu'],
});
const results = [];
try {
  for (const { name, skinId } of cases) {
    const entry = payloads.find(({ ship }) => ship.name === name);
    if (!entry) {
      results.push({ name, error: 'Ship is not in the catalogue' });
      continue;
    }
    const { ship, skinrs } = entry;
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (message.text().startsWith('Missing resource')) errors.push(message.text());
    });
    try {
      const skin =
        (skinId
          ? ship.skins.find((candidate) => candidate.id === skinId)
          : ship.skins.find((candidate) => candidate.dna.includes(':pattern?'))) ?? ship.skins[0];
      const design =
        skinrs.designs.find((candidate) =>
          candidate.layout.slots.some((slot) => slot.configuration.pattern),
        ) ?? skinrs.designs[0];
      await page.goto(`${BASE}/ships/${ship.id}${skin ? `?skin=${skin.id}` : ''}`);
      await page.locator('.boot').waitFor({ state: 'hidden', timeout: 45000 });
      const rail = page.locator('#liveries');
      await rail.scrollIntoViewIfNeeded();
      await page.waitForTimeout(1500);
      const ccpAlerts = await page.getByRole('alert').allTextContents();
      const row = { name, id: ship.id, ccp: skin?.name ?? 'Standard issue', ccpAlerts, errors };
      if (design) {
        // Different player designs can share a name; select the matching rail position.
        const duplicateIndex = skinrs.designs
          .slice(0, skinrs.designs.indexOf(design))
          .filter((candidate) => candidate.name === design.name).length;
        await rail
          .locator('.skin-rail')
          .last()
          .getByRole('button', { name: design.name, exact: true })
          .nth(duplicateIndex)
          .click();
        await page.waitForURL((url) => url.searchParams.get('skinr') === design.id);
        await page.waitForTimeout(3500);
        row.skinr = design.name;
        row.skinrAlerts = await page.getByRole('alert').allTextContents();
      }
      await page.screenshot({
        path: fileURLToPath(
          new URL(`render-audit-${ship.id}${skinId ? `-${skinId}` : ''}.png`, STORAGE),
        ),
      });
      results.push(row);
      console.log(JSON.stringify(row));
    } catch (error) {
      results.push({ name, error: String(error), errors });
      console.log(JSON.stringify(results.at(-1)));
    } finally {
      await page.close();
    }
  }
} finally {
  await browser.close();
}
const report = { checkedAt: new Date().toISOString(), results };
await writeFile(
  new URL(repairs ? 'skin-render-repairs.json' : 'skin-render-audit.json', STORAGE),
  JSON.stringify(report, null, 2),
);
if (
  results.some(
    (row) => row.error || row.errors?.length || row.ccpAlerts?.length || row.skinrAlerts?.length,
  )
)
  process.exitCode = 1;
