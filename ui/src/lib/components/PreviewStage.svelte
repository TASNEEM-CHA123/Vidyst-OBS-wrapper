<script lang="ts">
  import Icon from './Icon.svelte'
  import type { AspectTargets, Layout, StudioSnapshot } from '../types'

  let {
    layout,
    snapshot,
    aspects,
    live = true,
    connected = false,
    horizontalFrame = null,
    verticalFrame = null,
    verticalError = null,
    onProjector,
  }: {
    layout: Layout | null
    snapshot: StudioSnapshot | null
    aspects: AspectTargets
    live?: boolean
    connected?: boolean
    horizontalFrame?: string | null
    verticalFrame?: string | null
    verticalError?: string | null
    onProjector?: (target: 'horizontal' | 'vertical') => void
  } = $props()

  let horizontalSlots = $derived(layout?.slots.R16x9 ?? [])
  let verticalSlots = $derived(layout?.slots.R9x16 ?? [])
  let cameraVisible = $derived(snapshot?.camera_visible ?? true)
  let screenVisible = $derived(snapshot?.screen_visible ?? true)
  let backgroundRemoved = $derived(snapshot?.background_removal ?? false)
  let canvasStyle = $derived(
    snapshot?.background_mode === 'color'
      ? `background: ${snapshot.background_color}`
      : undefined,
  )

  function visible(source: string) {
    if (source === 'Camera') return cameraVisible
    if (source === 'Screen') return screenVisible
    return true
  }
</script>

<section class="stage" class:single={!(aspects.horizontal && aspects.vertical)}>
  <header class="stage-head">
    <div class="program">
      <span class="program-dot" class:standby={!live}></span>
      <span class="program-label">{live ? 'PROGRAM' : 'STANDBY'}</span>
      <span class="scene-name">{snapshot?.program_scene ?? 'No scene selected'}</span>
    </div>
    <div class="stage-meta">
      <span class="live-chip" class:on={connected && Boolean(horizontalFrame)}>
        <span class="live-dot"></span>
        {connected ? (horizontalFrame ? 'LIVE' : 'SYNC') : 'OFFLINE'}
      </span>
      {#if snapshot?.transition}
        <span class="meta-chip">{snapshot.transition} · {snapshot.transition_duration_ms} ms</span>
      {/if}
      <span class="meta-chip">16:9 · 1920 × 1080</span>
      <div class="projector-actions">
        <button
          type="button"
          class="icon-btn"
          title="Open horizontal projector"
          onclick={() => onProjector?.('horizontal')}
        >
          <Icon name="projector" size={15} />
          <span>16:9</span>
        </button>
        <button
          type="button"
          class="icon-btn"
          title="Open vertical projector"
          onclick={() => onProjector?.('vertical')}
        >
          <Icon name="projector" size={15} />
          <span>9:16</span>
        </button>
      </div>
    </div>
  </header>

  <div class="preview-wrap" class:pair={aspects.horizontal && aspects.vertical}>
    {#if aspects.horizontal}
      <div class="frame horizontal-frame">
        <div class="canvas-shell">
          <div class="canvas horizontal-canvas" style={canvasStyle}>
            {#if connected}
              {#if horizontalFrame}
                <img
                  class="live-frame"
                  src={horizontalFrame}
                  alt="Live OBS horizontal program output"
                  draggable="false"
                />
                <span class="live-badge"><span class="live-dot"></span>OBS LIVE</span>
              {:else}
                <div class="live-wait">
                  <span class="wait-spinner"></span>
                  <span>Waiting for the live OBS frame…</span>
                </div>
              {/if}
            {:else}
            <div class="grid"></div>
          {#each horizontalSlots as item (item.source + item.rect.x + item.rect.y)}
            {@const rect = item.rect}
            <div
              class="slot"
              class:screen={item.source === 'Screen'}
              class:camera={item.source === 'Camera'}
              class:hidden={!visible(item.source)}
              class:removed={item.source === 'Camera' && backgroundRemoved}
              style:left={`${rect.x * 100}%`}
              style:top={`${rect.y * 100}%`}
              style:width={`${rect.w * 100}%`}
              style:height={`${rect.h * 100}%`}
              style:border-radius={item.source === 'Screen'
                ? '10px'
                : (snapshot?.camera_radius ?? 0) >= 400
                  ? '50%'
                  : '14px'}
            >
              {#if item.source === 'Screen'}
                <div class="screen-ui">
                  <div class="screen-top">
                    <span class="dot red"></span><span class="dot amber"></span><span class="dot green"></span>
                    <span class="screen-title">{snapshot?.screen?.name ?? 'Display capture'}</span>
                  </div>
                  <div class="screen-body">
                    <div class="screen-sidebar">
                      <span></span><span></span><span></span><span></span>
                    </div>
                    <div class="screen-content">
                      <span class="content-title"></span>
                      <span class="content-line wide"></span>
                      <span class="content-line"></span>
                      <span class="content-line short"></span>
                      <div class="content-cards"><span></span><span></span><span></span></div>
                    </div>
                  </div>
                </div>
              {:else if item.source === 'Camera'}
                <div class="camera-bg"></div>
                <div class="camera-glow"></div>
                <div class="person">
                  <span class="head"></span>
                  <span class="shoulders"></span>
                </div>
                <span class="source-tag"><Icon name="camera" size={10} /> CAM</span>
                {#if backgroundRemoved}<span class="removed-tag">BG removed</span>{/if}
              {:else}
                <span class="source-tag">Background</span>
              {/if}
            </div>
          {/each}
          {#if !live}
            <div class="standby-shade">
              <span class="standby-title">Choose a layout</span>
              <span class="standby-copy">The preview updates as soon as you go on program.</span>
            </div>
          {/if}
            {/if}
          </div>
        </div>
        <div class="frame-foot">
          <span>Horizontal</span>
          <span>16:9 · 1920 × 1080</span>
        </div>
      </div>
    {/if}

    {#if aspects.vertical}
      <div class="frame vertical-frame">
        <div class="canvas-shell">
          <div class="canvas vertical-canvas" style={canvasStyle}>
            {#if connected}
              {#if verticalFrame}
                <img
                  class="live-frame"
                  src={verticalFrame}
                  alt="Live OBS vertical output"
                  draggable="false"
                />
                <span class="live-badge compact"><span class="live-dot"></span>LIVE</span>
              {:else}
                <div class="live-wait compact">
                  <span class="wait-spinner"></span>
                  <span>{verticalError ? 'Vertical live view unavailable' : 'Waiting for vertical…'}</span>
                </div>
              {/if}
            {:else}
            <div class="grid"></div>
          {#each verticalSlots as item (item.source + item.rect.x + item.rect.y)}
            {@const rect = item.rect}
            <div
              class="slot"
              class:screen={item.source === 'Screen'}
              class:camera={item.source === 'Camera'}
              class:hidden={!visible(item.source)}
              class:removed={item.source === 'Camera' && backgroundRemoved}
              style:left={`${rect.x * 100}%`}
              style:top={`${rect.y * 100}%`}
              style:width={`${rect.w * 100}%`}
              style:height={`${rect.h * 100}%`}
              style:border-radius={item.source === 'Screen'
                ? '7px'
                : (snapshot?.camera_radius ?? 0) >= 400
                  ? '50%'
                  : '11px'}
            >
              {#if item.source === 'Screen'}
                <div class="screen-ui">
                  <div class="screen-top">
                    <span class="dot red"></span><span class="dot amber"></span><span class="dot green"></span>
                  </div>
                  <div class="screen-body">
                    <div class="screen-sidebar">
                      <span></span><span></span><span></span>
                    </div>
                    <div class="screen-content">
                      <span class="content-title"></span>
                      <span class="content-line wide"></span>
                      <span class="content-line"></span>
                    </div>
                  </div>
                </div>
              {:else if item.source === 'Camera'}
                <div class="camera-bg"></div>
                <div class="camera-glow"></div>
                <div class="person">
                  <span class="head"></span>
                  <span class="shoulders"></span>
                </div>
                {#if backgroundRemoved}<span class="removed-tag">BG</span>{/if}
              {:else}
                <span class="source-tag">Background</span>
              {/if}
            </div>
          {/each}
          {#if !live}
            <div class="standby-shade compact">
              <span class="standby-title">Vertical</span>
              <span class="standby-copy">9:16 output preview</span>
            </div>
          {/if}
            {/if}
          </div>
        </div>
        <div class="frame-foot">
          <span>Vertical</span>
          <span>9:16 · 1080 × 1920</span>
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  .stage {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    padding: 14px 16px 10px;
    border-radius: var(--radius-lg);
    background:
      radial-gradient(circle at 50% -20%, rgba(118, 92, 255, 0.075), transparent 52%),
      var(--surface-1);
    border: 1px solid var(--border-soft);
    box-shadow: var(--shadow-panel);
    overflow: hidden;
  }

  .stage-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding-bottom: 11px;
    flex-wrap: wrap;
  }

  .program {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .program-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .program-dot.standby {
    background: var(--text-faint);
    box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.04);
  }

  .program-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.16em;
    color: var(--text-muted);
  }

  .scene-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    padding-left: 9px;
    margin-left: 1px;
    border-left: 1px solid var(--border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 220px;
  }

  .stage-meta {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }

  .meta-chip {
    font-size: 10px;
    color: var(--text-faint);
    white-space: nowrap;
  }

  .meta-chip + .meta-chip::before {
    content: '·';
    padding-right: 7px;
    color: var(--border-strong);
  }

  .projector-actions {
    display: flex;
    gap: 5px;
    margin-left: 3px;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 8px;
    border-radius: 7px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    transition: 130ms ease;
  }

  .icon-btn:hover {
    color: var(--text);
    border-color: var(--border-strong);
    background: var(--surface-3);
  }

  .preview-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: stretch;
    justify-content: center;
    gap: 16px;
  }

  .frame {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    min-width: 0;
  }

  .horizontal-frame {
    flex: 1;
  }

  .canvas-shell {
    flex: 1;
    min-height: 0;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    container-type: size;
  }

  .horizontal-canvas {
    width: min(100cqw, calc(100cqh * 16 / 9));
    height: auto;
    aspect-ratio: 16 / 9;
    flex: none;
  }

  .vertical-frame {
    width: clamp(136px, 20%, 204px);
    flex: none;
  }

  .vertical-canvas {
    height: min(100cqh, calc(100cqw * 16 / 9));
    width: auto;
    aspect-ratio: 9 / 16;
    flex: none;
  }

  .canvas {
    position: relative;
    background: #07090d;
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow:
      0 22px 48px -24px rgba(0, 0, 0, 0.9),
      0 0 0 1px rgba(0, 0, 0, 0.45);
    overflow: hidden;
    border-radius: 4px;
  }

  .live-frame {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    background: #05070a;
  }

  .live-wait {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 7px;
    color: var(--text-faint);
    font-size: 10px;
    background:
      radial-gradient(circle at 50% 42%, rgba(118, 92, 255, 0.08), transparent 50%),
      #07090d;
  }

  .live-wait.compact {
    gap: 5px;
    font-size: 8px;
    padding: 8px;
    text-align: center;
  }

  .wait-spinner {
    width: 15px;
    height: 15px;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.12);
    border-top-color: var(--accent);
    animation: spin 750ms linear infinite;
  }

  .live-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 22px;
    padding: 0 7px;
    border-radius: 6px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
    color: var(--text-faint);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  .live-chip.on {
    color: #6fe0b3;
    border-color: rgba(64, 217, 158, 0.24);
    background: rgba(28, 198, 132, 0.08);
  }

  .live-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: currentColor;
    box-shadow: 0 0 6px currentColor;
  }

  .live-badge {
    position: absolute;
    z-index: 4;
    top: 8px;
    right: 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 6px;
    border-radius: 5px;
    background: rgba(5, 7, 11, 0.66);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #7ce3b8;
    font-size: 8px;
    font-weight: 750;
    letter-spacing: 0.09em;
    backdrop-filter: blur(4px);
  }

  .live-badge.compact {
    top: 5px;
    right: 5px;
    font-size: 7px;
    padding: 2px 4px;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .grid {
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(rgba(255, 255, 255, 0.022) 1px, transparent 1px),
      linear-gradient(90deg, rgba(255, 255, 255, 0.022) 1px, transparent 1px);
    background-size: 4.5% 8%;
    opacity: 0.7;
    pointer-events: none;
  }

  .slot {
    position: absolute;
    overflow: hidden;
    transition:
      opacity 180ms ease,
      left 260ms cubic-bezier(0.22, 1, 0.36, 1),
      top 260ms cubic-bezier(0.22, 1, 0.36, 1),
      width 260ms cubic-bezier(0.22, 1, 0.36, 1),
      height 260ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .slot.hidden {
    opacity: 0;
  }

  .slot.screen {
    background:
      linear-gradient(122deg, rgba(84, 153, 255, 0.11), transparent 55%),
      #0d141d;
    border: 1px solid rgba(122, 165, 217, 0.33);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.02);
  }

  .screen-ui {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: linear-gradient(160deg, rgba(26, 42, 58, 0.75), rgba(9, 14, 20, 0.95));
  }

  .screen-top {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 18%;
    min-height: 10px;
    padding: 0 7%;
    background: rgba(6, 10, 14, 0.65);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    flex: none;
  }

  .dot.red {
    background: #ff6d6d;
  }

  .dot.amber {
    background: #f6b85f;
  }

  .dot.green {
    background: #58c98b;
  }

  .screen-title {
    margin-left: 6px;
    font-size: clamp(5px, 0.8vw, 9px);
    color: rgba(186, 204, 224, 0.6);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .screen-body {
    flex: 1;
    display: flex;
    min-height: 0;
    padding: 5% 6%;
    gap: 5%;
  }

  .screen-sidebar {
    width: 16%;
    display: flex;
    flex-direction: column;
    gap: 9%;
    padding-top: 2%;
  }

  .screen-sidebar span {
    display: block;
    height: clamp(2px, 1.5%, 5px);
    border-radius: 2px;
    background: rgba(125, 159, 197, 0.2);
  }

  .screen-sidebar span:first-child {
    background: rgba(127, 151, 255, 0.35);
    width: 80%;
  }

  .screen-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 5%;
    padding-top: 2%;
  }

  .content-title {
    display: block;
    width: 38%;
    height: clamp(3px, 2.6%, 9px);
    border-radius: 2px;
    background: rgba(154, 183, 215, 0.4);
  }

  .content-line {
    display: block;
    width: 68%;
    height: clamp(2px, 1.7%, 6px);
    border-radius: 2px;
    background: rgba(138, 162, 188, 0.18);
  }

  .content-line.wide {
    width: 88%;
  }

  .content-line.short {
    width: 45%;
  }

  .content-cards {
    display: flex;
    gap: 5%;
    margin-top: 3%;
    flex: 1;
    min-height: 0;
  }

  .content-cards span {
    flex: 1;
    border-radius: 4px;
    background: linear-gradient(150deg, rgba(94, 134, 180, 0.18), rgba(58, 83, 112, 0.08));
    border: 1px solid rgba(139, 168, 200, 0.08);
  }

  .slot.camera {
    background: linear-gradient(160deg, #242336, #11111a);
    border: 1px solid rgba(172, 151, 255, 0.32);
    box-shadow:
      inset 0 0 40px rgba(116, 72, 255, 0.12),
      0 8px 26px -14px rgba(0, 0, 0, 0.9);
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }

  .slot.removed {
    background-color: #222831;
    background-image:
      linear-gradient(45deg, #2b323d 25%, transparent 25%),
      linear-gradient(-45deg, #2b323d 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #2b323d 75%),
      linear-gradient(-45deg, transparent 75%, #2b323d 75%);
    background-size: 14px 14px;
    background-position:
      0 0,
      0 7px,
      7px -7px,
      -7px 0;
  }

  .camera-bg {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(circle at 48% 22%, rgba(173, 150, 255, 0.3), transparent 55%),
      linear-gradient(145deg, rgba(118, 89, 200, 0.24), rgba(28, 24, 42, 0.9));
  }

  .camera-glow {
    position: absolute;
    inset: 0;
    background: radial-gradient(ellipse at 50% 100%, rgba(126, 94, 255, 0.16), transparent 60%);
  }

  .person {
    position: relative;
    z-index: 1;
    width: 60%;
    height: 66%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
  }

  .head {
    width: 27%;
    aspect-ratio: 1;
    border-radius: 50%;
    background: linear-gradient(160deg, rgba(229, 220, 255, 0.72), rgba(157, 139, 204, 0.48));
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.22);
    margin-bottom: -3%;
    z-index: 1;
  }

  .shoulders {
    width: 72%;
    height: 42%;
    border-radius: 48% 48% 16% 16%;
    background: linear-gradient(165deg, rgba(203, 190, 248, 0.42), rgba(103, 88, 150, 0.35));
  }

  .source-tag {
    position: absolute;
    z-index: 2;
    left: 7px;
    bottom: 6px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 5px;
    border-radius: 4px;
    background: rgba(5, 7, 11, 0.58);
    border: 1px solid rgba(255, 255, 255, 0.09);
    color: rgba(231, 236, 247, 0.55);
    font-size: 7px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  .removed-tag {
    position: absolute;
    z-index: 2;
    right: 7px;
    bottom: 6px;
    padding: 2px 5px;
    border-radius: 4px;
    background: rgba(28, 198, 132, 0.14);
    border: 1px solid rgba(64, 217, 158, 0.25);
    color: #6fe0b3;
    font-size: 7px;
    font-weight: 700;
  }

  .standby-shade {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 5px;
    background: rgba(5, 7, 11, 0.62);
    backdrop-filter: blur(2px);
  }

  .standby-shade.compact {
    gap: 2px;
  }

  .standby-title {
    font-size: clamp(11px, 1.3vw, 17px);
    font-weight: 650;
    letter-spacing: -0.01em;
    color: rgba(238, 241, 249, 0.88);
  }

  .standby-copy {
    font-size: clamp(8px, 0.8vw, 10px);
    color: var(--text-faint);
  }

  .frame-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 2px 0;
    font-size: 9px;
    color: var(--text-faint);
    letter-spacing: 0.035em;
  }

  .frame-foot span:first-child {
    color: var(--text-muted);
    font-weight: 600;
  }

  .vertical-frame .frame-foot {
    font-size: 8px;
  }

  @media (max-width: 1240px) {
    .vertical-frame {
      width: 132px;
    }

    .meta-chip {
      display: none;
    }
  }

  @media (max-height: 760px) {
    .stage {
      padding: 10px 12px 7px;
    }

    .stage-head {
      padding-bottom: 8px;
    }

    .frame-foot {
      padding-top: 4px;
    }
  }
</style>
