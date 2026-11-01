<script lang="ts">
  import { invalidateAll } from '$app/navigation';
  import { LoaderCircle, Upload } from '@lucide/svelte';
  import type { SkinrImportReport } from '#lib/types.js';
  import { stage } from '#lib/stage.svelte.js';

  let { data } = $props();
  let selected = $state('');
  let busy = $state(false);
  let error = $state('');
  let report = $state<SkinrImportReport | null>(null);
  let query = $state('');
  let upload: HTMLInputElement;
  const files = $derived(data.imports.files.filter((file) => file.kind === 'designs'));
  const ships = $derived(
    data.imports.ships.filter((ship) => ship.name.toLowerCase().includes(query.toLowerCase())),
  );
  const number = new Intl.NumberFormat('en');
  const MAX_FILE_BYTES = 32 * 1024 * 1024;

  $effect(() => {
    stage.mode = 'hidden';
  });

  async function request(path: string, body: string): Promise<SkinrImportReport> {
    const response = await fetch(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
    });
    const result = await response.json().catch(() => null);
    if (!response.ok)
      throw new Error(result?.message ?? `Import failed (HTTP ${response.status}).`);
    return result;
  }

  async function loadFolder() {
    busy = true;
    error = '';
    report = null;
    try {
      report = await request(
        '/api/skinr/imports',
        JSON.stringify({ files: selected ? [selected] : [] }),
      );
      await invalidateAll();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function loadUploads(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const chosen = Array.from(input.files ?? []);
    if (!chosen.length) return;
    busy = true;
    error = '';
    report = null;
    const total = { files: 0, designs: 0, added: 0, already_imported: 0 };
    try {
      // Files are separate transactions; show progress if a later file fails.
      for (const file of chosen) {
        if (file.size > MAX_FILE_BYTES) throw new Error(`${file.name} exceeds the 32 MiB limit.`);
        // Sequential uploads bound memory and preserve completed-file counts.
        // oxlint-disable-next-line no-await-in-loop
        const result = await request('/api/skinr/imports/upload', await file.text());
        total.files += result.files;
        total.designs += result.designs;
        total.added += result.added;
        total.already_imported += result.already_imported;
        report = { ...total };
      }
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      input.value = '';
      await invalidateAll();
      busy = false;
    }
  }
</script>

<svelte:head><title>Import SKINR designs · skinr.space</title></svelte:head>

<main class="relative z-20 mx-auto min-h-screen max-w-6xl px-6 pt-36 pb-24 lg:px-12">
  <p class="eyebrow mb-4 text-muted-foreground">JSON imports</p>
  <h1 class="display mb-6 text-5xl sm:text-7xl">Your skin library.</h1>
  <p class="mb-12 max-w-xl text-lg text-muted-foreground">
    Load saved SKINR designs, try them on their ships, and export a selected design for EveJS.
    Imported designs stay in your library even when they’re no longer for sale.
  </p>
  <p class="mb-8 max-w-2xl text-sm text-muted-foreground">
    EveJS downloads use EveOffline’s portable .skinr.json format. Copy a download to your EveOffline
    data root’s skinr/designs folder, then run /skinr import "filename.skinr.json" in game. This
    creates a draft; component rights and licenses are not transferred.
  </p>

  <div class="glass mb-10 rounded-3xl p-6 sm:p-8">
    <h2 class="mb-3 text-xl font-semibold">Load designs</h2>
    <p class="mb-6 max-w-2xl text-sm text-muted-foreground">
      Choose a snapshot from the repository JSON folder or upload your own files. Repeated designs
      are counted once. Listing snapshots contain prices, not designs; use the details files.
      Uploads accept ESI design objects and EVE Ref details snapshots.
    </p>
    {#if data.imports.folder_available && files.length}
      <div class="mb-5 flex flex-col gap-3 sm:flex-row">
        <select
          aria-label="Design snapshot"
          bind:value={selected}
          disabled={busy}
          class="min-w-0 flex-1 rounded-xl bg-background px-4 py-3 text-sm text-foreground"
        >
          <option value="">All {files.length} design snapshots</option>
          {#each files as file (file.name)}<option value={file.name}>{file.name}</option>{/each}
        </select>
        <button
          type="button"
          disabled={busy}
          onclick={loadFolder}
          class="inline-flex items-center justify-center gap-2 rounded-xl bg-foreground px-5 py-3 text-sm font-medium text-background disabled:opacity-50"
        >
          {#if busy}<LoaderCircle class="size-4 animate-spin" />{:else}<Upload
              class="size-4"
            />{/if}
          Load from folder
        </button>
      </div>
    {:else}
      <p class="mb-5 text-sm text-muted-foreground">
        The repository JSON folder is unavailable or has no design files. You can upload JSON below.
      </p>
    {/if}
    <input
      bind:this={upload}
      class="hidden"
      type="file"
      accept=".json,application/json"
      multiple
      onchange={loadUploads}
      disabled={busy}
      aria-label="Upload design JSON files"
    />
    <button
      type="button"
      onclick={() => upload.click()}
      disabled={busy}
      class="rounded-xl bg-foreground/10 px-5 py-3 text-sm font-medium disabled:opacity-50"
      >Choose JSON files</button
    >
    <span class="ml-3 text-xs text-muted-foreground">Up to 32 MiB per file</span>
    {#if report}
      <p class="mt-6 text-sm" role="status">
        Added {number.format(report.added)} designs from {report.files}
        {report.files === 1 ? 'file' : 'files'}. {number.format(report.already_imported)} already in your
        library.
      </p>
    {/if}
    {#if error}<p class="mt-5 text-sm text-destructive" role="alert">{error}</p>{/if}
  </div>

  <div class="mb-7 flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
    <div>
      <h2 class="text-2xl font-semibold">Imported designs</h2>
      <p class="mt-2 text-sm text-muted-foreground">
        {number.format(data.imports.imported_designs)} designs across {data.imports.ships.length} ships.
        Open a ship and select a design to preview or export it.
      </p>
    </div>
    <input
      aria-label="Find an imported ship"
      placeholder="Find a ship…"
      bind:value={query}
      class="rounded-xl bg-foreground/5 px-4 py-3 text-sm outline-none focus:ring-2 focus:ring-foreground/20"
    />
  </div>
  <div class="grid gap-x-8 sm:grid-cols-2 lg:grid-cols-3">
    {#each ships as ship (ship.id)}
      <a
        href="/ships/{ship.id}?designs=imported#liveries"
        class="flex items-center justify-between border-b border-foreground/10 py-4 hover:text-muted-foreground"
      >
        <span class="font-medium">{ship.name}</span><span
          class="text-sm tabular-nums text-muted-foreground">{ship.designs}</span
        >
      </a>
    {:else}
      <p class="py-8 text-sm text-muted-foreground">
        {data.imports.imported_designs
          ? 'No ships match your search.'
          : 'Load a JSON snapshot to start your library.'}
      </p>
    {/each}
  </div>
</main>
