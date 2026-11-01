# SKINR for EveJS

[![CI](https://github.com/locvez/skinr-evejs/actions/workflows/ci.yml/badge.svg)](https://github.com/locvez/skinr-evejs/actions/workflows/ci.yml)
[![Release](https://github.com/locvez/skinr-evejs/actions/workflows/release.yml/badge.svg)](https://github.com/locvez/skinr-evejs/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-Axum-DEA584?logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-Svelte%205-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/docs/kit)
[![WebGPU](https://img.shields.io/badge/WebGPU-CarbonEngineJS-005A9C)](https://github.com/carbonenginejs/runtime)

Try on EVE Online ship skins on the real hull in 3D, right in the browser:
CCP skins, SKINR designs on the Paragon Hub, and imported JSON designs.
Export designs as native EveOffline/EveJS draft files.

Based on [Nicolas Kion's skinr.space](https://github.com/NicolasKion/skinr),
with fixes for skin coverage, JSON imports, and EveJS exports. The original
MIT license and attribution are preserved.

> [!NOTE]
> This is a rough first version and mostly vibe coded. It works, but it is
> an experiment, not a finished product: expect rough edges, missing
> pieces and things that change or break.

## Features

- Every ship in the game, grouped by class and by who builds it (the empires
  and the pirate and faction builders), straight from the SDE
- Each hull rendered with EVE's own shaders on WebGPU by
  [CarbonEngineJS](https://github.com/carbonenginejs/runtime), with every
  skin to try on it
- SKINR designs on sale, synced from ESI every 10 minutes, with their price
- Import EVE Ref/ESI JSON designs from files or a local folder
- Export native `.skinr.json` drafts for EveOffline/EveJS, with optional
  validation against your server's supported hulls and components
- A blueprint sheet per ship (side, top and front elevations) and its stats
- Related ships to jump to, with the 3D ship popping out of its card

## Stack

- API: Rust, [Axum](https://github.com/tokio-rs/axum) and
  [sqlx](https://github.com/launchbadge/sqlx) on Postgres. Ship and skin data
  come from CCP's Static Data Export, SKINR listings from ESI, and game files
  through a `/cdn` passthrough to CCP's client CDNs.
- Frontend: [SvelteKit](https://svelte.dev/docs/kit) (adapter-node), Svelte 5
  and Tailwind CSS 4

One origin serves both: Caddy (or the Vite dev proxy) sends `/api` and `/cdn`
to Axum and every page to SvelteKit.

## Run it

```sh
docker compose up --build       # http://localhost:5200
```

It starts Postgres, downloads and seeds the latest SDE when it changed, and
serves the app. Viewing ships needs a browser with WebGPU and BC texture
compression (desktop Chrome or Chromium; on Linux you may need
`--enable-unsafe-webgpu`).

For development, run Postgres in docker and the rest on the host:

```sh
docker compose up -d postgres
cp .env.example .env
cargo run --bin sde_import      # once, and after each SDE release
cargo run                       # API on 127.0.0.1:3100
cd frontend && npm install && npm run dev -- --port 5200
```

## Import JSON designs and export to EveJS

Open `/imports` (the **Import JSON** link in the site bar). Load one or all design
snapshots from `src/skinrJSON`, or choose local JSON files. The importer accepts
EVE Ref maps keyed by design ID, individual ESI designs, and arrays of designs.
Use `skinr-details-*.json`; listing snapshots contain offers rather than recipes.
The repository includes 70 design snapshots and 70 matching listing snapshots
in this folder. Use the design snapshots for imports; listing snapshots are
included for reference.
The local Docker stack mounts this folder read-only. `SKINR_JSON_DIR` changes the
folder for a native API process. Uploads are limited to 32 MiB per file.

Imports validate hulls, components and pattern transforms before committing.
Repeated IDs are deduplicated; conflicting recipes reject the whole file or
folder batch. Imported designs remain available without current Paragon Hub
listings, and the ship page offers **Imported**, **On sale**, and **All designs**
filters. No marketplace offers or ownership rights are created by an import.

Select a SKINR design and click **Export to EveJS**. The download is EveOffline's
native `eveoffline.skinr-design` version 1 format, with a `.skinr.json` filename.
It preserves component selections, pattern projections, transforms and mirroring,
and converts blend/projection modes to the native wire enums. Coating overrides
use the native zero-vector defaults, matching EveOffline's EVE Ref converter.
Retail hashes, creators, licenses and jobs are omitted from the portable draft.

In your EveOffline checkout, copy the downloaded file into the
configured data root's `skinr/designs/` directory (normally
`_local/skinr/designs/`), then run in game:

```text
/skinr import "downloaded-filename.skinr.json"
```

EveOffline validates against its installed hull/component data and creates a new
draft owned by the importing character. Sequencing still requires the appropriate
component rights and funds. Name, line-name and file-size limits are checked before
download; the target server also enforces its supported content and draft capacity.

To check exports against the installed EveOffline hull/component data, generate a
read-only capability snapshot from `frontend/`:

```powershell
node scripts/snapshot-evejs-target.ts C:/path/to/EveOffline
```

The API reads `storage/evejs/export-target.json` (or `EVEJS_EXPORT_PROFILE`). Docker
mounts this directory read-only. Unsupported designs remain viewable, while their
export button explains the target's restriction. Regenerate this snapshot after
updating EveOffline's static data. With no snapshot configured, downloads still
use the native format and explicitly say target content availability is unchecked.

## Layout

- `src/sde/` the SDE import: download, parse ships, groups, races and skins, seed Postgres.
- `src/server/` the axum router and the JSON API (`/api/ships`, `/api/ships/{id}`, `/api/health`).
- `src/cdn.rs` the `/cdn/binaries` and `/cdn/resources` passthrough to CCP's client CDNs.
  The browser runtime reads game files through it, because CCP's CloudFront serves
  cached copies without CORS headers to browsers.
- `migrations/` the Postgres schema.
- `frontend/` the SvelteKit app (Tailwind v4, shadcn-svelte tooling, an Apple-style dark
  design). `src/lib/viewer/` holds the WebGPU renderer and the shared engine;
  `src/lib/components/stage.svelte` is the persistent 3D stage that moves between the
  ship hero and the explorer's dock.
- `deploy/Caddyfile` the shared-origin routing, local and in production.

## Deploy

The optional production setup runs on Coolify from `compose.prod.yml`.
Manually run the **Release** workflow to build the api, frontend and proxy
images and push them to this repository's GHCR namespace. If configured,
it then calls the Coolify deploy webhook (`COOLIFY_WEBHOOK` and
`COOLIFY_TOKEN` repo secrets).
The SDE import runs before the api on each deploy. Thumbnails are rendered
locally and synced to the `skinr_thumbnails` volume (see the top of
`compose.prod.yml`).

## Checks

```sh
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
cd frontend && npm run check && npm run lint && npm run format:check && npm test
```

`node frontend/scripts/shot.js /ships/603 shot.png wait` screenshots a page of the
running app in Chromium. It opens a window, since headless Chromium usually has no
WebGPU adapter on Linux.

To audit the local skin catalogue on Windows, start the Docker stack, then run:

```powershell
./frontend/scripts/snapshot-skin-catalog.ps1
cd frontend
node scripts/audit-skins.ts
node scripts/audit-skin-dna.ts
node scripts/audit-skin-renders.mjs
```

The snapshot reads the cached official SDE and active database listings. The first
audit compares every ship's APIs and rendered skin selectors against that snapshot.
The second validates every appearance against the current client's SOF catalogue.
The last uses headless Edge with WebGPU to check CCP and SKINR selection on 24
representative hulls; pass ship names as arguments to check other hulls. JSON reports
and screenshots go to `storage/`. Run the snapshot and audits close together so
listing changes do not produce differences between the snapshot and the live API.

## Runtime workarounds

The runtime is pre-alpha. `frontend/src/lib/viewer/renderer.ts`,
`frontend/src/lib/viewer/resources.ts`, and `frontend/vite.config.ts` fill a few
gaps, each commented in place:

- The composed WebGPU render context is not exported; a Vite plugin resolves it.
- Class files are missing from the package's `sideEffects` list, so production
  builds would drop ship classes; a Vite plugin keeps every runtime module.
- The canvas cannot be attached to a `Tr2RenderTarget`, so the scene draws straight
  into the canvas pass, without post-processing or the nebula background.
- On that path depth is never cleared; the canvas target clears it every frame.
- Re-creating the device on resize double-frees a pooled texture, so resizes only
  reconfigure the surface.
- The driver needs a `CjsBatchManager`, and `.red` paths are read as `.black`.
- Some SDE factions and patterns exist only in the complete client SOF catalogue.
  Missing individual records are recovered from that catalogue on demand. These
  rare skins require a larger client-data download on their first load.

Lighting is still flat: the runtime does not drive sun lighting, shadows or
post-processing yet.

EVE Online and all related assets are the property of CCP hf. This project is not
affiliated with or endorsed by CCP.

## License

MIT, see [LICENSE](LICENSE). The EVE Online assets the app loads at runtime
are not covered by it.
