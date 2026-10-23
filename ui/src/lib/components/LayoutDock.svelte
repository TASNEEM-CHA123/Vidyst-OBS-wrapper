<script lang="ts">
  import LayoutThumb from './LayoutThumb.svelte'
  import type { AspectRatio, AspectTargets, Layout } from '../types'

  let {
    layouts,
    activeLayout,
    aspects,
    onSelect,
    disabled = false,
    busy = false,
  }: {
    layouts: Layout[]
    activeLayout: string | null
    aspects: AspectTargets
    onSelect: (id: string) => void
    disabled?: boolean
    busy?: boolean
  } = $props()

  let solo = $derived(layouts.filter((layout) => layout.group === 'Solo'))
  let screen = $derived(layouts.filter((layout) => layout.group === 'Screen'))
  let vertical = $derived(!aspects.horizontal && aspects.vertical)
  let primaryAspect = $derived<AspectRatio>(aspects.horizontal ? 'R16x9' : 'R9x16')
  let secondaryAspect = $derived<AspectRatio | null>(
    aspects.horizontal && aspects.vertical ? 'R9x16' : null,
  )
  let planLabel = $derived(
    aspects.horizontal && aspects.vertical
      ? '16:9 + 9:16 plans'
      : vertical
        ? '9:16 plan'
        : '16:9 plan',
  )
</script>

<section class="dock" class:vertical class:disabled={disabled || busy}>
  <header class="dock-head">
    <div class="dock-title">
      <span class="eyebrow">Layouts</span>
      <span class="dock-count">{layouts.length} ready</span>
    </div>
    <span class="dock-hint"
      >{busy
        ? 'Switching layout…'
        : `${planLabel} · Click a layout to send it to program`}</span
    >
  </header>

  <div class="groups">
    <div class="group solo">
      <div class="group-head">Solo</div>
      <div class="group-list">
        {#each solo as layout (layout.id)}
          <LayoutThumb
            {layout}
            aspect={primaryAspect}
            secondaryAspect={secondaryAspect}
            active={activeLayout === layout.id}
            onSelect={(id) => onSelect(id)}
            showLabel
          />
        {/each}
      </div>
    </div>

    <div class="group screen">
      <div class="group-head">Screen</div>
      <div class="group-list">
        {#each screen as layout (layout.id)}
          <LayoutThumb
            {layout}
            aspect={primaryAspect}
            secondaryAspect={secondaryAspect}
            active={activeLayout === layout.id}
            onSelect={(id) => onSelect(id)}
            showLabel
          />
        {/each}
      </div>
    </div>
  </div>
</section>

<style>
  .dock {
    flex: none;
    padding: 10px 16px 12px;
    border-radius: var(--radius-lg);
    background: var(--surface-1);
    border: 1px solid var(--border-soft);
    box-shadow: var(--shadow-panel);
    transition: opacity 160ms ease;
  }

  .dock.disabled {
    opacity: 0.55;
    pointer-events: none;
  }

  .dock-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 7px;
  }

  .dock-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .eyebrow {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .dock-count {
    font-size: 9px;
    color: var(--text-faint);
  }

  .dock-hint {
    font-size: 9px;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .groups {
    display: flex;
    gap: 12px;
    align-items: stretch;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 5px;
    flex: 1;
    min-width: 0;
  }

  .group.solo {
    flex: 0 0 calc((100% - 12px) / 8);
  }

  .group-head {
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .group-list {
    display: flex;
    gap: 7px;
    min-width: 0;
  }

  .group-list :global(.thumb) {
    flex: 1;
    min-width: 0;
  }

  .dock.vertical :global(.group-list .thumb) {
    max-width: 78px;
  }

  .dock.vertical .group.solo {
    flex: 0 0 calc((100% - 12px) / 8);
  }

  .dock.vertical .groups {
    gap: 14px;
  }

  .dock.vertical .group-list {
    gap: 8px;
    justify-content: flex-start;
  }

  @media (max-width: 1200px) {
    .dock {
      padding: 9px 12px 10px;
    }

    .dock-hint {
      display: none;
    }
  }
</style>
