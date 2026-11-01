<script lang="ts">
  // The site bar: one glass capsule floating at the top of every page. The
  // logo and a breadcrumb on the left (Ships, then the page's own title); on
  // the right the page's sections, with a pill sliding under the one in
  // view, and its reset button; last the theme switch. The bar never changes
  // shape while scrolling.
  import { ChevronRight, Moon, RotateCcw, Sun, Upload } from '@lucide/svelte';
  import Logo from '#lib/components/logo.svelte';
  import { nav } from '#lib/nav.svelte.js';
  import { theme } from '#lib/theme.svelte.js';

  let links = $state<Record<string, HTMLAnchorElement>>({});
  const pill = $derived.by(() => {
    const link = nav.active ? links[nav.active] : null;
    return link ? { left: link.offsetLeft, width: link.offsetWidth } : null;
  });

  function jump(event: MouseEvent, id: string) {
    const target = document.getElementById(id);
    if (!target) return;
    event.preventDefault();
    const top = target.getBoundingClientRect().top + window.scrollY;
    window.scrollTo({ top, behavior: 'smooth' });
  }
</script>

<header class="pointer-events-none fixed inset-x-0 top-3 z-40 px-3 lg:px-6">
  <nav
    class="bar pointer-events-auto relative flex h-12 items-center gap-2 rounded-full pr-2 pl-4"
    aria-label="Site"
  >
    <a href="/" class="flex items-center gap-2 text-foreground/90 hover:text-foreground">
      <Logo class="size-[1.35rem]" />
      <span class="text-sm font-semibold tracking-tight"
        >skinr<span class="font-normal text-foreground/45">.space</span></span
      >
    </a>

    {#if nav.title}
      <ChevronRight class="size-3.5 shrink-0 text-foreground/30" />
      <a href="/" class="crumb text-sm">Ships</a>
      <ChevronRight class="size-3.5 shrink-0 text-foreground/30" />
      <span class="truncate text-sm font-medium text-foreground">{nav.title}</span>
    {/if}

    {#if nav.center}
      <!-- The page's docked controls, sliding in when its own row scrolls away. -->
      <div
        class="center mx-4 hidden min-w-0 flex-1 justify-center lg:flex"
        class:shown={nav.centerShown}
        inert={!nav.centerShown}
      >
        {@render nav.center()}
      </div>
    {/if}

    <div class="ml-auto flex items-center gap-1">
      <a
        href="/imports"
        aria-label="Import JSON"
        title="Import JSON"
        class="flex items-center gap-2 rounded-full px-3 py-2 text-sm text-foreground/70 hover:text-foreground"
      >
        <Upload class="size-4" /><span class="hidden sm:inline">Import JSON</span>
      </a>
      {#if nav.sections.length}
        <div class="relative hidden items-center lg:flex">
          {#if pill}
            <span class="pill" style:left="{pill.left}px" style:width="{pill.width}px"></span>
          {/if}
          {#each nav.sections as section (section.id)}
            <a
              bind:this={links[section.id]}
              href="#{section.id}"
              class="section relative px-3.5 py-1.5 text-[0.8rem]"
              class:active={nav.active === section.id}
              aria-current={nav.active === section.id ? 'location' : undefined}
              onclick={(event) => jump(event, section.id)}
            >
              {section.label}
            </a>
          {/each}
        </div>
        <span class="mx-2 hidden h-5 w-px bg-foreground/10 lg:block"></span>
      {/if}

      {#if nav.reset}
        <button
          type="button"
          class="icon-button"
          title="Reset view"
          aria-label="Reset view"
          onclick={() => nav.reset?.()}
        >
          <RotateCcw class="size-4" />
        </button>
      {/if}
      <a
        href="https://github.com/carbonenginejs"
        class="px-3 py-1.5 text-[0.8rem] text-foreground/55 hover:text-foreground"
      >
        CarbonEngineJS
      </a>
      <button
        type="button"
        class="icon-button"
        title={theme.current === 'dark' ? 'Light mode' : 'Dark mode'}
        aria-label={theme.current === 'dark' ? 'Light mode' : 'Dark mode'}
        onclick={() => theme.toggle()}
      >
        {#if theme.current === 'dark'}
          <Sun class="size-4" />
        {:else}
          <Moon class="size-4" />
        {/if}
      </button>
    </div>
  </nav>
</header>

<style>
  /* A soft scrim behind the floating bar: content scrolling past fades out
     at the top edge instead of peeking out above the capsule. */
  header::before {
    content: '';
    position: absolute;
    inset: -0.75rem 0 auto;
    height: 5.5rem;
    background: linear-gradient(var(--background) 15%, transparent);
    pointer-events: none;
  }
  .bar {
    background: var(--glass-strong);
    backdrop-filter: saturate(160%) blur(28px);
    box-shadow:
      inset 0 0 0 1px var(--glass-edge),
      0 12px 40px -12px var(--shadow);
  }
  .center {
    opacity: 0;
    translate: 0 0.6rem;
    transition:
      opacity 0.35s var(--ease-apple),
      translate 0.5s var(--ease-apple);
  }
  .center.shown {
    opacity: 1;
    translate: 0 0;
  }
  .crumb {
    color: color-mix(in oklab, var(--foreground) 60%, transparent);
    transition: color 0.2s;
  }
  .crumb:hover {
    color: var(--foreground);
  }
  .section {
    color: color-mix(in oklab, var(--foreground) 55%, transparent);
    transition: color 0.3s var(--ease-apple);
  }
  .section:hover,
  .section.active {
    color: var(--foreground);
  }
  /* Slides under whichever section is in view. */
  .pill {
    position: absolute;
    top: 50%;
    height: 2rem;
    translate: 0 -50%;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 9%, transparent);
    transition:
      left 0.45s var(--ease-apple),
      width 0.45s var(--ease-apple);
  }
  .icon-button {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 999px;
    color: color-mix(in oklab, var(--foreground) 70%, transparent);
    transition:
      color 0.2s,
      background 0.2s;
  }
  .icon-button:hover {
    color: var(--foreground);
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
  }
</style>
