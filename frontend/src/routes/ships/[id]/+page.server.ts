import { error } from '@sveltejs/kit';
import { apiGet } from '#lib/server/api.js';
import type { ShipDetail, ShipSkinrs } from '#lib/types.js';
import type { EvejsTarget } from '#lib/evejs-export.js';
import type { PageServerLoad } from './$types';

/** What the page shows when the SKINR designs cannot be loaded. */
const NO_SKINRS: ShipSkinrs = { slot_materials: {}, designs: [], components: {} };

export const load: PageServerLoad = async ({ fetch, params }) => {
  if (!/^\d+$/.test(params.id)) error(404, 'Ship not found.');
  // The player designs are extra: the page loads without them.
  const skinrs = apiGet<ShipSkinrs>(fetch, `/api/ships/${params.id}/skinrs`)
    .then((designs) => ({ designs, unavailable: false }))
    .catch(() => ({ designs: NO_SKINRS, unavailable: true }));
  const [ship, designs, evejsTarget] = await Promise.all([
    apiGet<ShipDetail>(fetch, `/api/ships/${params.id}`),
    skinrs,
    apiGet<EvejsTarget | null>(fetch, '/api/skinr/evejs-target').catch(() => null),
  ]);
  return { ship, skinrs: designs.designs, skinrsUnavailable: designs.unavailable, evejsTarget };
};
