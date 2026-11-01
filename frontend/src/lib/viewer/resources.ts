// The game client's file index and the Space Object Factory, read in the
// browser through the API's /cdn passthrough (CCP's CDNs drop CORS headers
// for browsers often enough that direct requests cannot be relied on).
import { CjsFileIndexLibrary } from '@carbonenginejs/runtime/tools/fileindex';
import { EveSOF } from '@carbonenginejs/runtime/sof';
import { CjsBlackFormat } from '@carbonenginejs/runtime/resource/formats/black';
import { createSofSource, SOF_CATALOGUE } from './sof-source.ts';

/* eslint-disable @typescript-eslint/no-explicit-any */
export type FileIndexLibrary = any;
export type SpaceObjectFactory = any;

function provider() {
  const binaries = `${location.origin}/cdn/binaries`;
  return {
    game: 'Eve',
    id: 'ccp',
    defaultBuildRef: 'latest',
    remote: {
      metadataBaseURL: binaries,
      indexBaseURL: binaries,
      appBaseURL: binaries,
      resBaseURL: `${location.origin}/cdn/resources`,
    },
    clients: { tranquility: { metadataToken: 'TQ', aliases: ['tq'] } },
  };
}

export function openLibrary(build = 'latest'): Promise<FileIndexLibrary> {
  return CjsFileIndexLibrary.load(provider(), { build });
}

/** The URL behind a res:/ path. The client ships only compiled .black, so .red falls back to it. */
export function resolveUrl(library: FileIndexLibrary, path: string): string | null {
  const hit =
    library.Resolve(path) ??
    (path.endsWith('.red') ? library.Resolve(`${path.slice(0, -4)}.black`) : null);
  return hit?.sourceURL ?? null;
}

/** Fetches a res:/ path as bytes. */
export async function readBytes(library: FileIndexLibrary, path: string): Promise<Uint8Array> {
  const url = resolveUrl(library, path);
  if (!url) throw new Error(`Not in the file index: ${path}`);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP ${response.status} for ${path}`);
  return new Uint8Array(await response.arrayBuffer());
}

export async function openSof(
  library: FileIndexLibrary,
  read = (path: string) => readBytes(library, path),
): Promise<SpaceObjectFactory> {
  const source = createSofSource(
    (path) => resolveUrl(library, path) !== null,
    read,
    async () => {
      return CjsBlackFormat.readPayload(await read(SOF_CATALOGUE), {}).object;
    },
  );
  const sof = await new EveSOF().Register({
    lazyData: { source },
    resources: { exists: async (path: string) => !!library.Resolve(path) },
  });
  await sof.InitializeAsync();
  return sof;
}
