<script lang="ts">
  import type { AspectRatio, Layout } from '../types'

  let {
    layout,
    aspect,
    secondaryAspect = null,
    active = false,
    onSelect,
    showLabel = false,
  }: {
    layout: Layout
    aspect: AspectRatio
    secondaryAspect?: AspectRatio | null
    active?: boolean
    onSelect?: (id: string) => void
    showLabel?: boolean
  } = $props()

  let slots = $derived(layout.slots[aspect] ?? [])
  let secondarySlots = $derived(
    secondaryAspect && secondaryAspect !== aspect ? (layout.slots[secondaryAspect] ?? []) : [],
  )
  let vertical = $derived(aspect === 'R9x16')
  let dual = $derived(secondarySlots.length > 0)
</script>

<button
  type="button"
  class="thumb"
  class:vertical
  class:dual
  class:active
  aria-label={`Apply ${layout.name}${dual ? ' (16:9 and 9:16)' : ''}`}
  aria-pressed={active}
  onclick={() => onSelect?.(layout.id)}
>
  <div class="mini-canvas">
    {#each slots as item (item.source + item.rect.x + item.rect.y)}
      {@const rect = item.rect}
      <div
        class="mini-slot"
        class:screen={item.source === 'Screen'}
        class:camera={item.source === 'Camera'}
        class:background={item.source === 'Background'}
        style:left={`${rect.x * 100}%`}
        style:top={`${rect.y * 100}%`}
        style:width={`${rect.w * 100}%`}
        style:height={`${rect.h * 100}%`}
        style:border-radius={item.source === 'Screen' ? '3px' : `${Math.min(item.radius_pct, 50)}%`}
      >
        {#if item.source === 'Camera'}
          <span class="mini-camera-head"></span>
          <span class="mini-camera-body"></span>
        {:else}
          <span class="mini-screen-bar"></span>
          <span class="mini-screen-line"></span>
          <span class="mini-screen-line short"></span>
        {/if}
      </div>
    {/each}
    {#if dual}
      <div class="secondary-canvas" aria-hidden="true">
        {#each secondarySlots as item (item.source + item.rect.x + item.rect.y)}
          {@const rect = item.rect}
          <div
            class="mini-slot"
            class:screen={item.source === 'Screen'}
            class:camera={item.source === 'Camera'}
            class:background={item.source === 'Background'}
            style:left={`${rect.x * 100}%`}
            style:top={`${rect.y * 100}%`}
            style:width={`${rect.w * 100}%`}
            style:height={`${rect.h * 100}%`}
            style:border-radius={item.source === 'Screen' ? '2px' : `${Math.min(item.radius_pct, 50)}%`}
          >
            {#if item.source === 'Camera'}
              <span class="mini-camera-head"></span>
              <span class="mini-camera-body"></span>
            {:else}
              <span class="mini-screen-bar"></span>
              <span class="mini-screen-line"></span>
              <span class="mini-screen-line short"></span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
  {#if showLabel}
    <span class="thumb-label">{layout.name}</span>
  {/if}
  {#if active}
    <span class="active-check">
      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
        <path d="m5 12 4.5 4.5L19 7" />
      </svg>
    </span>
  {/if}
</button>

<style>
  .thumb {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    border-radius: 9px;
    border: 1px solid var(--border-soft);
    background: #0b0f14;
    padding: 4px;
    cursor: pointer;
    transition:
      border-color 140ms ease,
      background 140ms ease,
      transform 140ms ease;
    overflow: hidden;
  }

  .thumb:hover {
    border-color: #485061;
    background: #111720;
    transform: translateY(-1px);
  }

  .thumb.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, #0b0f14);
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent),
      0 10px 24px -16px var(--accent);
  }

  .thumb.vertical {
    aspect-ratio: 9 / 16;
  }

  .mini-canvas {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: 5px;
    overflow: hidden;
    background:
      linear-gradient(135deg, rgba(255, 255, 255, 0.025), transparent 45%),
      radial-gradient(circle at 75% 20%, rgba(110, 76, 255, 0.09), transparent 50%),
      #080b10;
  }

  .mini-canvas::before {
    content: '';
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(rgba(255, 255, 255, 0.022) 1px, transparent 1px),
      linear-gradient(90deg, rgba(255, 255, 255, 0.022) 1px, transparent 1px);
    background-size: 11px 11px;
    pointer-events: none;
  }

  .secondary-canvas {
    position: absolute;
    top: 3px;
    right: 3px;
    height: 58%;
    aspect-ratio: 9 / 16;
    overflow: hidden;
    border-radius: 3px;
    border: 1px solid rgba(169, 157, 255, 0.6);
    background: #080b10;
    box-shadow: 0 4px 10px -4px rgba(0, 0, 0, 0.8);
    z-index: 2;
  }

  .secondary-canvas .mini-slot {
    padding: 0;
  }

  .secondary-canvas .mini-screen-line {
    height: 1px;
  }

  .thumb.dual .active-check {
    left: 6px;
    right: auto;
  }

  .thumb.dual .thumb-label {
    max-width: calc(100% - 38px);
  }

  .mini-slot {
    position: absolute;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
  }

  .mini-slot.screen {
    background-color: #4b4863;
    background-image: linear-gradient(115deg, rgba(255, 255, 255, 0.07), transparent 58%);
    border: 1px solid rgba(196, 190, 231, 0.14);
    padding: 3px;
  }

  .mini-slot.camera {
    background-color: #6b5b4b;
    background-image: radial-gradient(circle at 45% 28%, rgba(242, 222, 198, 0.22), transparent 55%);
    border: 1px solid rgba(240, 217, 190, 0.16);
  }

  .mini-camera-head {
    width: 20%;
    aspect-ratio: 1;
    border-radius: 50%;
    background: rgba(246, 229, 209, 0.72);
    flex: none;
  }

  .mini-camera-body {
    width: 44%;
    height: 26%;
    border-radius: 999px 999px 24% 24%;
    background: rgba(239, 219, 196, 0.58);
    flex: none;
  }

  .mini-screen-bar {
    position: absolute;
    top: 8%;
    left: 9%;
    width: 23%;
    height: 4%;
    border-radius: 2px;
    background: rgba(223, 219, 245, 0.42);
  }

  .mini-screen-line {
    width: 58%;
    height: 3px;
    border-radius: 2px;
    background: rgba(226, 222, 244, 0.24);
  }

  .mini-screen-line.short {
    width: 37%;
    background: rgba(226, 222, 244, 0.16);
  }

  .thumb-label {
    position: absolute;
    left: 7px;
    bottom: 5px;
    font-size: 9px;
    font-weight: 600;
    color: rgba(236, 239, 247, 0.72);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: calc(100% - 26px);
    pointer-events: none;
  }

  .active-check {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #0b0c10;
    background: var(--accent);
    box-shadow: 0 0 12px color-mix(in srgb, var(--accent) 60%, transparent);
  }
</style>
