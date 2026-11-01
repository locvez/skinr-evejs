import { apiGet } from '#lib/server/api.js';
import type { SkinrImportStatus } from '#lib/types.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch }) => ({
  imports: await apiGet<SkinrImportStatus>(fetch, '/api/skinr/imports'),
});
