<script lang="ts">
  import Icon from './Icon.svelte'
  import type { AspectTargets, ConnectionStatus, StudioSnapshot } from '../types'

  let {
    status,
    snapshot,
    aspects,
    activeLayoutName,
    busy = '',
    onToggleAspect,
    onRecord,
    onLive,
    onSplit,
    onProjector,
    onSettings,
    onAudio,
    onEdit,
  }: {
    status: ConnectionStatus
    snapshot: StudioSnapshot | null
    aspects: AspectTargets
    activeLayoutName: string | null
    busy?: string
    onToggleAspect: (target: 'horizontal' | 'vertical') => void
    onRecord: () => void
    onLive: () => void
    onSplit: () => void
    onProjector: (target: 'horizontal' | 'vertical') => void
    onSettings: () => void
    onAudio: () => void
    onEdit: () => void
  } = $props()

  let projectorOpen = $state(false)
  let recording = $derived(snapshot?.recording ?? false)
  let streaming = $derived(snapshot?.streaming ?? false)
  let recordTargets = $derived(
    snapshot?.recording_horizontal && snapshot?.recording_vertical
      ? 'H+V'
      : snapshot?.recording_vertical
        ? 'V'
        : snapshot?.recording_horizontal
          ? 'H'
          : '',
  )
  let streamTargets = $derived(
    snapshot?.streaming_horizontal && snapshot?.streaming_vertical
      ? 'H+V'
      : snapshot?.streaming_vertical
        ? 'V'
        : snapshot?.streaming_horizontal
          ? 'H'
          : '',
  )
  let recordTargetLabel = $derived(
    recordTargets === 'H+V'
      ? '16:9 + 9:16'
      : recordTargets === 'V'
        ? '9:16'
        : recordTargets === 'H'
          ? '16:9'
          : 'selected canvases',
  )
  let streamTargetLabel = $derived(
    streamTargets === 'H+V'
      ? '16:9 + 9:16'
      : streamTargets === 'V'
        ? '9:16'
        : streamTargets === 'H'
          ? '16:9'
          : 'selected canvases',
  )
  let statusLabel = $derived(
    streaming ? (recording ? 'LIVE + REC' : 'LIVE') : recording ? 'REC' : 'IDLE',
  )

  function projector(target: 'horizontal' | 'vertical') {
    projectorOpen = false
    onProjector(target)
  }
</script>

<header class="topbar">
  <div class="brand">
    <div class="logo">
      <svg width="19" height="19" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path d="M4 4.5h5.1l2.9 9.7 2.9-9.7H20L14.7 20h-5.4L4 4.5Z" fill="currentColor" />
      </svg>
    </div>
    <div class="wordmark">
      <span class="wordmark-name">Vidyst</span>
      <span class="wordmark-tag">STUDIO</span>
    </div>
  </div>

  <div class="center">
    <div class="aspect-group">
      <span class="aspect-label">Output</span>
      <button
        type="button"
        class="aspect-btn"
        class:active={aspects.horizontal}
        aria-pressed={aspects.horizontal}
        onclick={() => onToggleAspect('horizontal')}
      >16:9</button>
      <button
        type="button"
        class="aspect-btn"
        class:active={aspects.vertical}
        aria-pressed={aspects.vertical}
        onclick={() => onToggleAspect('vertical')}
      >9:16</button>
      <select class="aspect-more" disabled title="1:1 and 4:5 layouts arrive in V1.1">
        <option>1:1 / 4:5</option>
      </select>
    </div>

    <div class="scene-pill">
      <Icon name="layout" size={12} />
      <span>{activeLayoutName ?? 'No layout on program'}</span>
    </div>
  </div>

  <div class="actions">
    <div class="connection" title={status.message}>
      <span
        class="conn-dot"
        class:online={status.connected}
        class:warn={status.connected && !status.bootstrap_ready}
        class:error={status.state === 'error'}
      ></span>
      <span class="conn-text">
        {#if status.state === 'connecting'}
          Connecting
        {:else if status.connected}
          OBS {status.obs_version ?? ''}
        {:else}
          Offline
        {/if}
      </span>
    </div>

    <span
      class="status-pill"
      class:live={streaming}
      class:rec={recording && !streaming}
      title={recording || streaming
        ? `${recording ? `Recording ${recordTargetLabel}` : ''}${recording && streaming ? ' · ' : ''}${streaming ? `Streaming ${streamTargetLabel}` : ''}`
        : 'No output is active'}
    >
      <span class="status-dot"></span>
      {statusLabel}
    </span>

    <button
      type="button"
      class="action record"
      class:active={recording}
      disabled={busy === 'record'}
      onclick={onRecord}
      title={recording ? `Stop recording ${recordTargetLabel}` : 'Start recording for the selected canvases'}
    >
      <span class="record-dot"></span>
      <span>{recording ? snapshot?.recording_timecode ?? 'REC' : 'Record'}</span>
      {#if recording && recordTargets}
        <span class="target-chip">{recordTargets}</span>
      {/if}
    </button>

    {#if recording}
      <button
        type="button"
        class="icon-action split"
        title="Split recording now (both active canvases)"
        disabled={busy === 'split'}
        onclick={onSplit}
      >
        <Icon name="scissors" size={15} />
      </button>
    {/if}

    <button
      type="button"
      class="action live"
      class:active={streaming}
      disabled={busy === 'live'}
      onclick={onLive}
      title={streaming ? `End stream ${streamTargetLabel}` : 'Go live on the selected canvases'}
    >
      <Icon name="stream" size={15} />
      <span>{streaming ? snapshot?.stream_timecode ?? 'LIVE' : 'Go live'}</span>
      {#if streaming && streamTargets}
        <span class="target-chip">{streamTargets}</span>
      {/if}
    </button>

    <button type="button" class="icon-action" title="Open the audio mixer" onclick={onAudio}>
      <Icon name="headphones" size={17} />
    </button>

    <button type="button" class="icon-action" title="Open quick customization" onclick={onEdit}>
      <Icon name="sliders" size={17} />
    </button>

    <div class="projector-wrap">
      <button
        type="button"
        class="icon-action"
        class:active={projectorOpen}
        title="Open projector"
        onclick={() => (projectorOpen = !projectorOpen)}
      >
        <Icon name="projector" size={17} />
        <Icon name="chevron-down" size={11} />
      </button>
      {#if projectorOpen}
        <div class="projector-menu">
          <button type="button" onclick={() => projector('horizontal')}>
            <span class="menu-icon"><Icon name="screen" size={14} /></span>
            <span><strong>Horizontal projector</strong><small>Program output · 16:9</small></span>
          </button>
          <button type="button" onclick={() => projector('vertical')}>
            <span class="menu-icon"><Icon name="camera" size={14} /></span>
            <span><strong>Vertical projector</strong><small>Aitum vertical canvas · 9:16</small></span>
          </button>
        </div>
      {/if}
    </div>

    <button type="button" class="icon-action" title="Settings" onclick={onSettings}>
      <Icon name="settings" size={17} />
    </button>
  </div>
</header>

<style>
  .topbar {
    flex: none;
    height: 58px;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 18px;
    background: color-mix(in srgb, var(--surface-1) 90%, transparent);
    border-bottom: 1px solid var(--border-soft);
    backdrop-filter: blur(14px);
    position: relative;
    z-index: 20;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    flex: none;
    min-width: 176px;
  }

  .logo {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    color: #f3efff;
    background:
      radial-gradient(circle at 28% 20%, rgba(255, 255, 255, 0.24), transparent 45%),
      linear-gradient(145deg, #8f77ff, #5b38d6);
    box-shadow:
      0 4px 14px -6px rgba(124, 87, 255, 0.7),
      inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  }

  .wordmark {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }

  .wordmark-name {
    font-size: 15px;
    font-weight: 680;
    letter-spacing: -0.03em;
    color: var(--text);
  }

  .wordmark-tag {
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.18em;
    color: var(--text-faint);
  }

  .center {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }

  .aspect-group {
    display: flex;
    align-items: center;
    padding: 2px;
    border-radius: 8px;
    background: var(--surface-0);
    border: 1px solid var(--border-soft);
    flex: none;
  }

  .aspect-label {
    padding: 0 7px 0 6px;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .aspect-btn {
    height: 24px;
    min-width: 38px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-faint);
    font-size: 9.5px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    transition: 130ms ease;
  }

  .aspect-btn:hover {
    color: var(--text-muted);
  }

  .aspect-btn.active {
    color: var(--text);
    background: var(--surface-3);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
  }

  .aspect-more {
    height: 24px;
    max-width: 94px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font-size: 8px;
    padding: 0 4px;
    opacity: 0.55;
  }

  .scene-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 0 9px;
    height: 26px;
    border-radius: 7px;
    color: var(--text-faint);
    font-size: 9.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-left: 1px solid var(--border);
  }

  .scene-pill span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: none;
    margin-left: auto;
  }

  .connection {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-right: 5px;
    color: var(--text-faint);
    font-size: 9px;
    white-space: nowrap;
  }

  .conn-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #59606d;
  }

  .conn-dot.online {
    background: var(--success);
    box-shadow: 0 0 0 2px rgba(64, 211, 156, 0.12);
  }

  .conn-dot.warn {
    background: var(--warning);
    box-shadow: 0 0 0 2px rgba(245, 183, 86, 0.12);
  }

  .conn-dot.error {
    background: var(--danger);
    box-shadow: 0 0 0 2px rgba(255, 94, 94, 0.12);
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 23px;
    padding: 0 7px;
    border-radius: 5px;
    border: 1px solid var(--border-soft);
    background: var(--surface-0);
    color: var(--text-faint);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.09em;
    white-space: nowrap;
  }

  .status-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #5b626f;
  }

  .status-pill.rec {
    color: #ff9b9b;
    border-color: rgba(255, 99, 99, 0.2);
    background: rgba(255, 82, 82, 0.07);
  }

  .status-pill.rec .status-dot {
    background: #ff6d6d;
    box-shadow: 0 0 6px rgba(255, 88, 88, 0.55);
  }

  .status-pill.live {
    color: #f4efff;
    border-color: rgba(143, 112, 255, 0.38);
    background: rgba(128, 92, 255, 0.16);
  }

  .status-pill.live .status-dot {
    background: #a88aff;
    box-shadow: 0 0 7px rgba(150, 117, 255, 0.7);
  }

  .action {
    height: 31px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 0 11px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    transition: 130ms ease;
    white-space: nowrap;
  }

  .action:hover:not(:disabled) {
    border-color: var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
  }

  .action:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .target-chip {
    display: inline-flex;
    align-items: center;
    height: 15px;
    padding: 0 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
    color: inherit;
    font-size: 7.5px;
    font-weight: 750;
    letter-spacing: 0.06em;
    opacity: 0.82;
  }

  .record-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ff7070;
    box-shadow: 0 0 0 2px rgba(255, 100, 100, 0.11);
  }

  .record.active {
    color: #ffd9d9;
    border-color: rgba(255, 101, 101, 0.3);
    background: rgba(255, 86, 86, 0.13);
  }

  .record.active .record-dot {
    animation: pulse 1.6s ease-in-out infinite;
  }

  .live {
    color: #ddd6ff;
    border-color: rgba(132, 105, 255, 0.28);
    background: rgba(126, 92, 255, 0.12);
  }

  .live:hover:not(:disabled) {
    border-color: rgba(152, 127, 255, 0.45);
    background: rgba(134, 99, 255, 0.18);
    color: #f1edff;
  }

  .live.active {
    color: #fff;
    background: linear-gradient(135deg, #8b66ff, #6740e8);
    border-color: transparent;
    box-shadow: 0 4px 18px -8px rgba(133, 90, 255, 0.85);
  }

  .projector-wrap {
    position: relative;
  }

  .icon-action {
    height: 31px;
    min-width: 31px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    padding: 0 7px;
    border-radius: 7px;
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    transition: 130ms ease;
  }

  .icon-action:hover,
  .icon-action.active {
    color: var(--text);
    border-color: var(--border);
    background: var(--surface-2);
  }

  .projector-menu {
    position: absolute;
    top: calc(100% + 7px);
    right: 0;
    width: 238px;
    padding: 4px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    box-shadow: 0 18px 44px -14px rgba(0, 0, 0, 0.85);
    z-index: 40;
  }

  .projector-menu button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .projector-menu button:hover {
    background: var(--surface-3);
  }

  .menu-icon {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    color: #a89afa;
    background: rgba(137, 108, 255, 0.1);
    flex: none;
  }

  .projector-menu strong {
    display: block;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
  }

  .projector-menu small {
    display: block;
    margin-top: 1px;
    color: var(--text-faint);
    font-size: 8.5px;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  @media (max-width: 1200px) {
    .brand {
      min-width: 0;
    }

    .wordmark-tag,
    .aspect-label,
    .scene-pill {
      display: none;
    }
  }

  @media (max-width: 1040px) {
    .connection {
      display: none;
    }
  }
</style>
