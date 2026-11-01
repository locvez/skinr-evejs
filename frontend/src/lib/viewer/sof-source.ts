// Some live SDE names exist only in CCP's complete SOF catalogue. Keep those
// exact records available without retaining the much larger complete catalogue.
export const SOF_CATALOGUE = 'res:/dx9/model/spaceobjectfactory/data.black';
const SOF_ROOT = 'res:/dx9/model/spaceobjectfactory/';
const RECORD_KINDS = ['hull', 'faction', 'race', 'material', 'pattern', 'layout'] as const;
type SofRecord = { name: string; [key: string]: unknown };
type SofCatalogue = Partial<Record<(typeof RECORD_KINDS)[number], SofRecord[]>>;

export function createSofSource(
  indexed: (path: string) => boolean,
  read: (path: string) => Promise<Uint8Array>,
  readCatalogue: () => Promise<SofCatalogue>,
) {
  let missing: Promise<Map<string, SofRecord>> | null = null;
  return async (path: string) => {
    if (indexed(path)) return read(path);
    if (
      !/^res:\/dx9\/model\/spaceobjectfactory\/(hulls|factions|races|materials|patterns|layouts)\/[^/]+\.black$/.test(
        path,
      )
    ) {
      throw new Error(`Not in the file index: ${path}`);
    }
    missing ??= readCatalogue()
      .then((catalogue) => {
        const records = new Map<string, SofRecord>();
        for (const kind of RECORD_KINDS) {
          for (const record of catalogue[kind] ?? []) {
            const key = `${SOF_ROOT}${kind}s/${record.name.toLowerCase()}.black`;
            if (!indexed(key)) records.set(key, record);
          }
        }
        return records;
      })
      .catch((error) => {
        missing = null;
        throw error;
      });
    const record = (await missing).get(path);
    if (!record) throw new Error(`Not in the client SOF catalogue: ${path}`);
    return { object: record };
  };
}
