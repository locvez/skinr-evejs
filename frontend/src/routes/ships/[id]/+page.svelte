<script lang="ts">
  import { ChevronDown, Download, LoaderCircle } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Blueprint from '#lib/components/blueprint.svelte';
  import EveText from '#lib/components/eve-text.svelte';
  import FactionEmblem from '#lib/components/faction-emblem.svelte';
  import GameImage from '#lib/components/game-image.svelte';
  import SectionStage from '#lib/components/section-stage.svelte';
  import SkinRail from '#lib/components/skin-rail.svelte';
  import { skinrIcon, skinrLook } from '#lib/skinr.js';
  import { evejsExport, evejsFilename } from '#lib/evejs-export.js';
  import SheetMargin from '#lib/components/sheet-margin.svelte';
  import ShipRail from '#lib/components/ship-rail.svelte';
  import SkinIcon from '#lib/components/skin-icon.svelte';
  import StageAlert from '#lib/components/stage-alert.svelte';
  import { firstSentence, raceTint, shipBuilders, typeRender } from '#lib/eve.js';
  import { leapTo } from '#lib/leap.svelte.js';
  import { nav } from '#lib/nav.svelte.js';
  import { CENTER, stage, type StagePlacement } from '#lib/stage.svelte.js';
  import {
    BLUEPRINT_POSE,
    FRONT_POSE,
    HERO_POSE,
    SIDE_POSE,
    type CameraPose,
  } from '#lib/viewer/poses.js';
  import type { ShipSummary } from '#lib/types.js';

  let { data } = $props();
  const ship = $derived(data.ship);

  /** The selected skin's id from `?skin=`, or null for the ship's own livery. */
  const skinId = $derived(Number(page.url.searchParams.get('skin')) || null);
  const skin = $derived(ship.skins.find((candidate) => candidate.id === skinId) ?? null);
  /** The chosen player design from `?skinr=`, which replaces the livery. */
  const skinrs = $derived(data.skinrs);
  const designFilter = $derived(page.url.searchParams.get('designs') ?? 'all');
  const visibleDesigns = $derived(
    skinrs.designs.filter((design) =>
      designFilter === 'imported'
        ? design.imported
        : designFilter === 'listed'
          ? design.listings > 0
          : true,
    ),
  );
  let exportError = $state('');
  const skinrId = $derived(page.url.searchParams.get('skinr'));
  const skinrDesign = $derived(skinrs.designs.find((design) => design.id === skinrId) ?? null);
  const exportIssue = $derived.by(() => {
    if (!skinrDesign) return null;
    try {
      evejsExport(ship, skinrDesign, skinrs, data.evejsTarget);
      return null;
    } catch (cause) {
      return cause instanceof Error ? cause.message : String(cause);
    }
  });
  const look = $derived(
    skinrDesign ? skinrLook(ship.dna, skinrDesign, skinrs.components, skinrs.slot_materials) : null,
  );
  const dna = $derived(look?.dna ?? skin?.dna ?? ship.dna);
  const tint = $derived(raceTint(ship.race?.id));

  const summary = $derived<ShipSummary>({
    id: ship.id,
    name: ship.name,
    group_id: ship.group.id,
    race_id: ship.race?.id ?? null,
    faction_id: ship.faction?.id ?? null,
    meta_group_id: ship.meta_group?.id ?? null,
    tech_level: ship.tech_level,
    dna: ship.dna,
    colors: ship.colors,
  });

  // The stage shows this ship in the chosen livery; where it sits is up to
  // the chapter effect below.
  $effect(() => {
    stage.morphId = summary.id;
    stage.show(summary, dna, stage.direction, look?.pattern ?? null);
  });

  function selectSkin(id: number | null, scrollTop = false, direction?: 1 | -1) {
    if (id === skinId) return;
    // The carousel slides the new livery in from the side it sits on in the
    // list, or from the side an arrow key points (stepping wraps around).
    const order = liveries.map((livery) => livery.id);
    stage.direction = direction ?? (order.indexOf(id) >= order.indexOf(skinId) ? 1 : -1);
    const url = new URL(page.url.href);
    url.searchParams.delete('skinr');
    if (id === null) url.searchParams.delete('skin');
    else url.searchParams.set('skin', String(id));
    goto(url, { replace: true, reset: false });
    if (scrollTop) window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  /**
   * Shows a player design; it slides in from the side it sits on in the list,
   * or from the side an arrow key points.
   */
  function selectSkinr(design: { id: string }, direction?: 1 | -1) {
    if (design.id === skinrId) return;
    const order = visibleDesigns.map((candidate) => candidate.id);
    const current = skinrId === null ? -1 : order.indexOf(skinrId);
    stage.direction = direction ?? (order.indexOf(design.id) >= current ? 1 : -1);
    const url = new URL(page.url.href);
    url.searchParams.delete('skin');
    url.searchParams.set('skinr', design.id);
    goto(url, { replace: true, reset: false });
  }

  /**
   * The page reads as chapters. Each one frames the ship its own way: where
   * the stage sits (beside the chapter's text on wide screens) and the camera
   * angle. The chapter crossing the middle of the screen is the active one.
   */
  function filterDesigns(filter: string) {
    const url = new URL(page.url.href);
    if (filter === 'all') url.searchParams.delete('designs');
    else url.searchParams.set('designs', filter);
    goto(url, { replace: true, reset: false });
  }

  function exportDesign() {
    if (!skinrDesign) return;
    exportError = '';
    try {
      const bundle = evejsExport(ship, skinrDesign, skinrs, data.evejsTarget);
      const url = URL.createObjectURL(
        new Blob([JSON.stringify(bundle, null, 2) + '\n'], { type: 'application/json' }),
      );
      const link = document.createElement('a');
      link.href = url;
      link.download = evejsFilename(skinrDesign.name, skinrDesign.id);
      document.body.append(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 0);
    } catch (cause) {
      exportError = cause instanceof Error ? cause.message : String(cause);
    }
  }

  type Chapter = 'hero' | 'profile' | 'blueprint' | 'liveries' | 'more';
  const RIGHT: StagePlacement = { x: 0.2, scale: 0.8 };
  /**
   * How each chapter frames the main ship. From the blueprint on it glides
   * into the drawing's side elevation and stays there; the livery section
   * shows its own view of the ship (a second render), so both can be on
   * screen at once without either moving.
   */
  const CHAPTERS: Record<Chapter, { placement: StagePlacement; pose: CameraPose; dim: number }> = {
    hero: { placement: CENTER, pose: HERO_POSE, dim: 0 },
    profile: { placement: RIGHT, pose: SIDE_POSE, dim: 0 },
    blueprint: { placement: RIGHT, pose: BLUEPRINT_POSE, dim: 0 },
    liveries: { placement: RIGHT, pose: BLUEPRINT_POSE, dim: 0 },
    more: { placement: RIGHT, pose: BLUEPRINT_POSE, dim: 0 },
  };

  /** Below this width there is no room beside the text: the ship stays centred, dimmed. */
  const SIDE_BY_SIDE_MIN_WIDTH = 1024;
  const NARROW_TEXT_DIM = 0.75;

  let chapter = $state<Chapter>('hero');
  let innerWidth = $state(1440);
  const wide = $derived(innerWidth >= SIDE_BY_SIDE_MIN_WIDTH);
  const framing = $derived(CHAPTERS[chapter]);
  /** From the blueprint down, the main ship sits in the drawing. */
  const docked = $derived(chapter === 'blueprint' || chapter === 'liveries' || chapter === 'more');
  const placement = $derived(wide ? framing.placement : CENTER);

  /**
   * The drawing on the sheet: this ship in the selected livery once drafted.
   * While a new livery is being drafted the previous livery's drawing stays
   * up, so the sheet keeps its size and the page does not jump.
   */
  const drawing = $derived.by(() => {
    const blueprint = stage.blueprint;
    if (!blueprint) return null;
    const sameShip = blueprint.dna.split(':')[0] === dna.split(':')[0];
    return blueprint.dna === dna || sameShip ? blueprint.drawing : null;
  });

  $effect(() => {
    stage.placement = placement;
    stage.pose = framing.pose;
    stage.hologram = docked ? 1 : 0;
    stage.dim =
      !wide && !docked && chapter !== 'hero' ? Math.max(framing.dim, NARROW_TEXT_DIM) : framing.dim;
    stage.dockName = 'blueprint';
    stage.blueprintFit = docked && drawing ? { halfLength: drawing.halfLength } : null;
    stage.mode = docked ? 'dock' : 'hero';
  });
  $effect(() => () => {
    stage.blueprintFit = null;
    stage.dockName = 'dock';
  });

  // Draft the blueprint while the reader is still above it.
  $effect(() => {
    stage.wantBlueprint = true;
    return () => (stage.wantBlueprint = false);
  });

  /**
   * Where the blueprint section sits in the page. Its paper is painted
   * behind the stage (the section itself is above it, with the page's text),
   * so the docked ship shows on it.
   */
  let paper = $state<{ top: number; height: number } | null>(null);
  function paperOf(node: HTMLElement) {
    const measure = () => {
      const rect = node.getBoundingClientRect();
      paper = { top: rect.top + window.scrollY, height: rect.height };
    };
    measure();
    // Its place moves with anything above it, so watch the whole page too.
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    observer.observe(document.body);
    return { destroy: () => observer.disconnect() };
  }

  /** Marks a section as a chapter; it becomes active while it crosses the middle of the screen. */
  function chapterOf(node: HTMLElement, name: Chapter) {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) chapter = name;
      },
      { rootMargin: '-50% 0px -50% 0px' },
    );
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  const liveries = $derived([
    { id: null, name: 'Standard issue', colors: ship.colors, icon_url: null, icon_colors: null },
    ...ship.skins,
  ]);
  const liveryName = $derived(skin?.name ?? 'Standard issue');
  /** Whether the ship has anything to wear besides its own colors. */
  // The livery chapter's two rails: the game's own skins, the players' designs.
  const liveryKey = (id: number | null) => `skin-${id ?? 'standard'}`;
  const skinItems = $derived(
    liveries.map((livery) => ({ key: liveryKey(livery.id), name: livery.name, livery })),
  );
  const designItems = $derived(
    visibleDesigns.map((design) => ({ key: design.id, name: design.name, design })),
  );
  const plex = new Intl.NumberFormat('en-US');
  /**
   * Arrow keys step through a carousel, wrapping around: in the livery
   * chapter, the rail of the look on the ship (the player designs while one
   * is worn), the liveries everywhere else.
   */
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement)
      return;
    const step = event.key === 'ArrowRight' ? 1 : -1;
    const wrap = (index: number, length: number) => (index + step + length) % length;
    event.preventDefault();
    if (chapter === 'liveries' && skinrDesign) {
      const designs = visibleDesigns;
      if (!designs.length) return;
      selectSkinr(designs[wrap(designs.indexOf(skinrDesign), designs.length)], step);
      return;
    }
    const order = liveries.map((livery) => livery.id);
    selectSkin(order[wrap(order.indexOf(skinId), order.length)] ?? null, false, step);
  }

  /**
   * The picker is a carousel: its track slides so the selected livery sits
   * in the middle, however many there are, and the ends fade out.
   */
  let track = $state<HTMLDivElement>();
  let pickerWidth = $state(0);
  const trackShift = $derived.by(() => {
    const index = liveries.findIndex((livery) => livery.id === skinId);
    const selected = track?.children[index] as HTMLElement | undefined;
    if (!selected || !pickerWidth) return 0;
    return pickerWidth / 2 - (selected.offsetLeft + selected.offsetWidth / 2);
  });

  const builders = $derived(shipBuilders(data.catalog));
  const builderOf = (other: ShipSummary) => builders.of(other).name;
  const groupNames = $derived(new Map(data.catalog.groups.map((group) => [group.id, group.name])));
  const metaGroups = $derived(new Map(data.catalog.meta_groups.map((meta) => [meta.id, meta])));
  const factionLogos = $derived(
    new Map(data.catalog.factions.map((faction) => [faction.id, faction.logo_url])),
  );

  /** The most cards a rail lists. */
  const RAIL_LIMIT = 24;
  /** A hull's model family: the first part of its SOF hull name (gc4 for gc4_t2a). */
  const hullFamily = (shipDna: string) => shipDna.split(':')[0].split('_')[0];

  /**
   * Related ships, in rails from closest to loosest: the same frame, the same
   * class, the same builder. A ship appears only in the first rail it fits.
   */
  const rails = $derived.by(() => {
    const listed = new Set([ship.id]);
    const byTechThenName = (a: ShipSummary, b: ShipSummary) =>
      (a.tech_level ?? 0) - (b.tech_level ?? 0) || a.name.localeCompare(b.name);
    const take = (fits: (other: ShipSummary) => boolean) => {
      const ships = data.catalog.ships
        .filter((other) => !listed.has(other.id) && fits(other))
        .sort(byTechThenName)
        .slice(0, RAIL_LIMIT);
      for (const other of ships) listed.add(other.id);
      return ships;
    };
    const family = hullFamily(ship.dna);
    const builder = ship.faction
      ? {
          title: `More from ${ship.faction.name}`,
          note: 'Every class they field',
          fits: (other: ShipSummary) => other.faction_id === ship.faction?.id,
        }
      : {
          title: `More ${ship.race?.name ?? 'unaffiliated'} ships`,
          note: 'Every class they field',
          fits: (other: ShipSummary) => other.race_id === (ship.race?.id ?? null),
        };
    return [
      {
        title: 'Same frame',
        note: `Built on the ${ship.name}'s hull, fitted for other roles`,
        ships: take((other) => hullFamily(other.dna) === family),
      },
      {
        title: `More ${ship.group.name.toLowerCase()}s`,
        note: 'The same class from other builders',
        ships: take((other) => other.group_id === ship.group.id),
        showClass: false,
      },
      { title: builder.title, note: builder.note, ships: take(builder.fits) },
    ].filter((rail) => rail.ships.length);
  });

  const loading = $derived(stage.status === 'starting' || stage.status === 'loading');

  /** Which bar link each chapter lights up; the hero lights none. */
  const SECTION_OF: Record<Chapter, string | null> = {
    hero: null,
    profile: 'profile',
    blueprint: 'blueprint',
    liveries: 'liveries',
    more: 'related',
  };

  // The site bar names this ship, links its sections and resets its view.
  $effect(() => {
    nav.title = ship.name;
    nav.sections = [
      ...(ship.description ? [{ id: 'profile', label: 'Profile' }] : []),
      { id: 'blueprint', label: 'Blueprint' },
      { id: 'liveries', label: 'Skins' },
      ...(rails.length ? [{ id: 'related', label: 'Related' }] : []),
    ];
    nav.reset = () => stage.resetView?.();
  });
  $effect(() => {
    nav.active = SECTION_OF[chapter];
  });
  $effect(() => () => nav.clear());
</script>

<svelte:head>
  <title>{ship.name} skins · skinr.space</title>
</svelte:head>

<svelte:window bind:innerWidth onkeydown={onKeydown} />

<!-- Behind the stage: a soft spotlight and the faction's emblem as tinted
     light. Fixed and at z-0, so the transparent canvas shows it. Kept quiet on
     purpose; the ship is the subject. -->
<div
  class="backdrop pointer-events-none fixed inset-0 z-0 overflow-hidden"
  style:opacity={docked ? 0 : 1 - stage.dim}
  style:transform="translateX({placement.x * 100}vw) scale({placement.scale})"
>
  <div
    class="absolute inset-0"
    style:background="radial-gradient(45% 40% at 50% 52%, color-mix(in oklab, {tint} 14%, transparent),
    transparent 70%)"
  ></div>
  {#if ship.faction?.logo_url}
    {#key ship.faction.logo_url}
      <div
        class="emblem-motion absolute top-1/2 left-1/2 size-[min(120vh,105vw)] -translate-x-1/2 -translate-y-1/2"
        style:opacity={1 - stage.hologram}
      >
        <FactionEmblem url={ship.faction.logo_url} {tint} class="emblem-in size-full" />
      </div>
    {/key}
  {/if}
</div>

<main class="overflow-x-clip">
  {#if paper}
    <div
      class="blueprint-paper pointer-events-none absolute inset-x-0 z-0"
      style:top="{paper.top}px"
      style:height="{paper.height}px"
    >
      <SheetMargin edge="top" />
      <SheetMargin edge="bottom" />
    </div>
  {/if}
  <!-- The hero: everything in it lets pointer events through to the stage
       except its controls. -->
  <section
    class="pointer-events-none relative z-20 flex h-dvh flex-col"
    style:--tint={tint}
    use:chapterOf={'hero'}
  >
    <div
      class="mx-auto mt-auto flex w-full max-w-[90rem] flex-wrap items-end justify-between gap-8 px-6 pb-16"
    >
      <div class="intro max-w-xl">
        <p class="eyebrow mb-3" style:color={tint}>
          {[builderOf(summary), ship.group.name, ship.meta_group?.name].filter(Boolean).join(' · ')}
        </p>
        <h1
          class="display text-[clamp(3.25rem,7.5vw,6.5rem)]"
          style:view-transition-name="ship-name"
        >
          <span class="text-tint-sheen">{ship.name}</span>
        </h1>
        {#if ship.description}
          <p class="mt-4 text-lg leading-relaxed text-foreground/70">
            {firstSentence(ship.description)}
          </p>
        {/if}
      </div>

      <div class="intro intro-late pointer-events-auto flex max-w-full flex-col items-end gap-3">
        <p class="text-sm text-foreground/80">
          <kbd class="mr-2 hidden text-[0.7rem] text-muted-foreground/70 lg:inline">← →</kbd>
          <span class="text-muted-foreground">Skin</span>
          <span class="mx-1.5 text-foreground/20">/</span>{liveryName}
        </p>
        <div
          class="picker glass w-[min(100%,26rem)] overflow-clip rounded-full py-2"
          bind:clientWidth={pickerWidth}
        >
          <div
            bind:this={track}
            class="picker-track flex w-max gap-2"
            style:transform="translateX({trackShift}px)"
          >
            {#each liveries as livery (livery.id ?? 'standard')}
              <button
                type="button"
                class="shrink-0 rounded-full p-0.5 transition-transform duration-300 hover:scale-110"
                title={livery.name}
                aria-label={livery.name}
                aria-pressed={skinId === livery.id}
                onclick={() => selectSkin(livery.id)}
              >
                <SkinIcon
                  iconUrl={livery.icon_url}
                  iconColors={livery.icon_colors}
                  colors={livery.colors}
                  selected={skinId === livery.id}
                  class="size-8"
                />
              </button>
            {/each}
          </div>
        </div>
      </div>
    </div>

    <!-- A broken ship glitches; this says why. -->
    <div class="pointer-events-auto absolute top-32 right-6 lg:right-10">
      <StageAlert name={ship.name} />
    </div>

    <div class="absolute inset-x-0 bottom-5 flex justify-center">
      {#if loading}
        <span class="flex items-center gap-2 text-xs text-muted-foreground">
          <LoaderCircle class="size-3.5 animate-spin" />
          {stage.status === 'starting' ? 'Starting the engine' : `Assembling the ${ship.name}`}
        </span>
      {:else if stage.status === 'error'}
        <!-- The alert above says what went wrong. -->
      {:else}
        <span class="cue flex flex-col items-center text-[0.7rem] text-muted-foreground">
          Scroll <ChevronDown class="size-4" />
        </span>
      {/if}
    </div>
  </section>

  <!-- Below the fold every chapter keeps the ship in view: its text takes one
       half of the screen, the stage moves to the other. -->
  <div class="relative z-20" style:--tint={tint}>
    {#if ship.description}
      <section id="profile" class="chapter" use:chapterOf={'profile'}>
        <div class="chapter-text">
          <p class="reveal eyebrow mb-4" style:color={tint}>Profile</p>
          <h2 class="reveal display mb-8 text-[clamp(2.25rem,4vw,3.5rem)]">
            <span class="text-sheen">{ship.faction?.name ?? ship.race?.name ?? 'Unaffiliated'}</span
            >
            <span class="block text-muted-foreground">{ship.group.name}</span>
          </h2>
          <div class="reveal description text-[1.0625rem] leading-[1.7] text-foreground/75">
            <EveText segments={ship.description_rich} />
          </div>
        </div>
      </section>
    {/if}

    <!-- The blueprint is a sheet of its own: solid blueprint paper from edge to
         edge, set hard against the chapters around it, and dark in either theme. -->
    <section
      id="blueprint"
      class="theme-dark min-h-dvh px-4 py-24 lg:px-10"
      use:chapterOf={'blueprint'}
      use:paperOf
    >
      <div class="mx-auto max-w-[90rem]">
        <p class="reveal eyebrow mb-4 text-[var(--blueprint-ink)]">Blueprint</p>
        <h2 class="reveal display mb-10 text-[clamp(2.25rem,4vw,3.5rem)] text-white">
          Everything it takes to build one.
        </h2>
        <Blueprint {ship} {drawing} build={data.catalog.sde_build} built={docked} />
      </div>
    </section>

    <section id="liveries" class="chapter relative" use:chapterOf={'liveries'}>
      <SectionStage
        key="liveries"
        pose={FRONT_POSE}
        {tint}
        logoUrl={ship.faction?.logo_url ?? null}
        {wide}
        placement={RIGHT}
      />
      <div class="chapter-text relative">
        <p class="reveal eyebrow mb-4" style:color={tint}>Skins</p>
        <h2 class="reveal display mb-4 text-[clamp(2.25rem,4vw,3.5rem)]">
          <span class="text-sheen"
            >{liveries.length + skinrs.designs.length} looks. Try them on.</span
          >
        </h2>
        <p class="reveal mb-10 max-w-md text-muted-foreground">
          Browse in-game skins for the {ship.name}, Paragon Hub designs, and imported JSON designs.
          Select a look to see it on the hull.
        </p>

        <!-- The look on the ship, from either rail. -->
        <div class="reveal mb-6 min-h-14">
          <p class="truncate text-2xl font-semibold tracking-tight">
            {skinrDesign?.name ?? liveryName}
          </p>
          <p class="mt-1 truncate text-sm text-muted-foreground">
            {#if skinrDesign}
              SKINR by <span class="text-foreground"
                >{skinrDesign.creator.name ?? 'a capsuleer'}</span
              >{#if skinrDesign.imported}
                · Imported JSON{/if}{#if skinrDesign.min_price_plex !== null}{` · from ${plex.format(skinrDesign.min_price_plex)} PLEX`}{/if}
            {:else if skin}
              In-game skin
            {:else}
              The hull's own colors
            {/if}
          </p>
          {#if skinrDesign}
            <button
              type="button"
              onclick={exportDesign}
              disabled={Boolean(exportIssue)}
              class="mt-4 inline-flex items-center gap-2 rounded-xl bg-foreground/10 px-4 py-2.5 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Download class="size-4" /> Export to EveJS
            </button>
            <p class="mt-2 text-xs text-muted-foreground">
              {exportIssue ??
                (data.evejsTarget
                  ? 'EveOffline .skinr.json · creates an importable draft'
                  : 'EveOffline .skinr.json · target content availability has not been checked')}
            </p>
            {#if exportError}<p role="alert" class="mt-2 text-sm text-destructive">
                {exportError}
              </p>{/if}
          {/if}
        </div>

        <div class="space-y-4">
          <SkinRail
            label="In-game"
            items={skinItems}
            selected={skinrDesign ? null : liveryKey(skinId)}
            onselect={(item) => selectSkin(item.livery.id)}
          >
            {#snippet icon(item, selected)}
              <SkinIcon
                iconUrl={item.livery.icon_url}
                iconColors={item.livery.icon_colors}
                colors={item.livery.colors}
                {selected}
                class="size-14"
              />
            {/snippet}
          </SkinRail>
          <div class="flex flex-wrap items-center gap-2" aria-label="Filter SKINR designs">
            {#each [{ key: 'all', label: 'All designs' }, { key: 'imported', label: 'Imported' }, { key: 'listed', label: 'On sale' }] as filter (filter.key)}
              <button
                type="button"
                aria-pressed={designFilter === filter.key}
                onclick={() => filterDesigns(filter.key)}
                class="rounded-full px-3 py-1.5 text-xs text-muted-foreground aria-pressed:bg-foreground/10 aria-pressed:text-foreground"
                >{filter.label}</button
              >
            {/each}
            <a href="/imports" class="ml-auto text-xs text-muted-foreground hover:text-foreground"
              >Import JSON</a
            >
          </div>
          {#if designItems.length}
            <SkinRail
              label="SKINR"
              items={designItems}
              selected={skinrDesign?.id ?? null}
              onselect={(item) => selectSkinr(item.design)}
            >
              {#snippet icon(item, selected)}
                {@const look = skinrIcon(item.design, skinrs.components)}
                <SkinIcon
                  colors={look.colors}
                  patterns={look.patterns}
                  {selected}
                  class="size-14"
                />
              {/snippet}
            </SkinRail>
          {:else}
            <p class="text-sm text-muted-foreground" role="status">
              {#if data.skinrsUnavailable}
                SKINR designs couldn’t be loaded. Reload the page to try again.
              {:else}
                No {designFilter === 'imported'
                  ? 'imported'
                  : designFilter === 'listed'
                    ? 'listed'
                    : 'SKINR'} designs are available for this hull yet.
              {/if}
            </p>
          {/if}
        </div>
      </div>
    </section>
    {#if rails.length}
      <section
        id="related"
        class="mx-auto max-w-[90rem] px-6 pt-[20vh] pb-24"
        use:chapterOf={'more'}
      >
        <p class="reveal eyebrow mb-4" style:color={tint}>Keep browsing</p>
        <h2 class="reveal display mb-14 text-[clamp(2.25rem,4vw,3.5rem)]">
          <span class="text-sheen">More hulls to dress up.</span>
        </h2>
        <div class="space-y-14">
          {#each rails as rail (rail.title)}
            <ShipRail
              title={rail.title}
              note={rail.note}
              ships={rail.ships}
              {groupNames}
              {builderOf}
              {metaGroups}
              {factionLogos}
              showClass={rail.showClass ?? true}
              onpick={leapTo}
            />
          {/each}
        </div>
      </section>
    {/if}
  </div>
</main>

<style>
  .blueprint-paper {
    background: var(--blueprint-paper);
  }
  /* The paper lights up the dark chapters next to it, like a lit table. */
  .blueprint-paper::before,
  .blueprint-paper::after {
    content: '';
    position: absolute;
    inset-inline: 0;
    height: 9rem;
    pointer-events: none;
  }
  .blueprint-paper::before {
    bottom: 100%;
    background: linear-gradient(
      to top,
      color-mix(in oklab, var(--blueprint-paper) 45%, transparent),
      transparent
    );
  }
  .blueprint-paper::after {
    top: 100%;
    background: linear-gradient(
      to bottom,
      color-mix(in oklab, var(--blueprint-paper) 45%, transparent),
      transparent
    );
  }
  /* The livery carousel: the ends fade so liveries slide in and out of view. */
  .picker {
    mask-image: linear-gradient(90deg, transparent, black 18%, black 82%, transparent);
  }
  .picker-track {
    transition: transform 0.6s var(--ease-apple);
  }
  .backdrop {
    transition:
      transform 0.9s cubic-bezier(0.65, 0, 0.35, 1),
      opacity 0.9s var(--ease-apple);
  }
  /* A chapter: a full screen whose text takes the left half (or the right
     half with .chapter-right); the ship fills the other. */
  .chapter {
    display: flex;
    align-items: center;
    min-height: 100dvh;
    max-width: 90rem;
    margin-inline: auto;
    padding: 6rem 1.5rem;
  }
  .chapter-text {
    width: 100%;
    max-width: 36rem;
  }
  @media (min-width: 1024px) {
    .chapter {
      padding-inline: 3rem;
    }
    .chapter-text {
      width: 42%;
    }
  }
  /* Long lore reads better in short paragraphs than in one wall. */
  .description :global(br + br) {
    display: block;
    content: '';
    margin-top: 0.9em;
  }
  .emblem-motion {
    transition: opacity 0.9s var(--ease-apple);
  }
  .emblem-motion :global(.emblem-in) {
    animation: emblem-in 2.4s var(--ease-out-expo) both;
  }

  .intro {
    animation: intro 1.1s var(--ease-out-expo) 0.25s both;
  }
  .intro-late {
    animation-delay: 0.45s;
  }

  .cue {
    animation: cue 2.4s ease-in-out infinite;
  }

  @keyframes emblem-in {
    from {
      opacity: 0;
      transform: scale(1.12);
      filter: blur(12px);
    }
  }
  @keyframes intro {
    from {
      opacity: 0;
      transform: translateY(24px);
      filter: blur(6px);
    }
  }
  @keyframes cue {
    0%,
    100% {
      transform: translateY(0);
      opacity: 0.6;
    }
    50% {
      transform: translateY(5px);
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .emblem-motion :global(.emblem-in),
    .intro,
    .cue {
      animation: none;
    }
  }
</style>
