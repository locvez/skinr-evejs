// Read-only snapshot of EveOffline's installed import capabilities. Never opens its runtime DB.
// Usage (from frontend/): node scripts/snapshot-evejs-target.ts C:/path/to/EveOffline
import { createRequire } from 'node:module';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

const target = process.argv[2];
if (!target) throw new Error('Supply the EveOffline checkout path.');
const require = createRequire(path.resolve(target, 'package.json'));
const authority = require('./server/src/services/ship/shipSkinComponentAuthority');
const {
  getShipSkinSequencingPolicy,
} = require('./server/src/services/ship/shipSkinSequencingAuthority');
const { resolveItemByTypeID } = require('./server/src/services/inventory/itemTypeRegistry');
const policy = getShipSkinSequencingPolicy();
const ships: Record<string, number[]> = {};
const slots: Record<string, number[]> = {};
for (const [id, group] of Object.entries(policy.shipGroupIDByTypeID)) {
  if (
    !policy.thresholdsByGroupID[group as string] ||
    Number(resolveItemByTypeID(Number(id))?.categoryID) !== 6
  )
    continue;
  const configuration = authority.getShipSkinSlotConfiguration(Number(id));
  if (configuration?.slotIDs.length) ships[id] = configuration.slotIDs;
}
for (const id of new Set(Object.values(ships).flat()))
  slots[id] = authority.getShipSkinSlot(id).allowedComponentCategoryIDs;
const components = Object.fromEntries(
  authority.listShipSkinComponents().map((component: any) => [
    component.componentID,
    {
      category: component.categoryID,
      available: component.associatedItems.length > 0,
      disallowed_slots: component.disallowedSlotIDs,
    },
  ]),
);
const result = {
  format: 'eveoffline.skinr-export-target',
  version: 1,
  generated_at: new Date().toISOString(),
  target: 'EveOffline',
  ships,
  slots,
  components,
};
const output = new URL('../../storage/evejs/', import.meta.url);
await mkdir(output, { recursive: true });
await writeFile(new URL('export-target.json', output), JSON.stringify(result));
console.log(
  JSON.stringify({
    target: path.resolve(target),
    ships: Object.keys(ships).length,
    components: Object.keys(components).length,
    output: new URL('export-target.json', output).pathname,
  }),
);
