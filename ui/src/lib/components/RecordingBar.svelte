<script lang="ts">
  import Icon from './Icon.svelte'
  import type { StudioSnapshot } from '../types'

  let {
    snapshot,
    busy = '',
    disabled = false,
    onChangeFolder,
    onOpenFolder,
  }: {
    snapshot: StudioSnapshot | null
    busy?: string
    disabled?: boolean
    onChangeFolder: () => void
    onOpenFolder: () => void
  } = $props()

  let directory = $derived(snapshot?.recording_directory ?? '')
  let lastFile = $derived(
    snapshot?.last_recording_path?.split(/[\\/]/).pop() ?? '',
  )
  let format = $derived(
    snapshot?.recording_format === 'hybrid_mp4'
      ? 'Hybrid MP4'
      : (snapshot?.recording_format ?? 'MP4').toUpperCase(),
  )
  let targetLabel = $derived(
    snapshot?.recording_horizontal && snapshot?.recording_vertical
      ? '16:9 + 9:16'
      : snapshot?.recording_vertical
        ? '9:16'
        : snapshot?.recording_horizontal
          ? '16:9'
          : '',
  )
</script>

<div class="recording-bar" class:live={snapshot?.recording}>
  <div class="recording-info">
    <span class="recording-icon" class:active={snapshot?.recording}>
      <Icon name={snapshot?.recording ? 'record' : 'folder'} size={15} />
    </span>
    <div class="recording-copy">
      <span class="recording-label">
        {snapshot?.recording ? 'RECORDING TO' : 'RECORDINGS SAVE TO'}
        {#if snapshot?.recording && targetLabel}
          <span class="recording-target">{targetLabel}</span>
        {/if}
      </span>
      <span class="recording-path" title={directory}>
        {directory || 'Waiting for the OBS recording folder…'}
      </span>
    </div>
    {#if lastFile}
      <div class="last-file" title={snapshot?.last_recording_path ?? ''}>
        <span class="last-label">LAST FILE</span>
        <span class="last-name">{lastFile}</span>
      </div>
    {:else}
      <div class="last-file empty">
        <span class="last-label">LAST FILE</span>
        <span class="last-name">No recording saved this session</span>
      </div>
    {/if}
  </div>

  <div class="recording-actions">
    <span class="format-chip" title="OBS recording format">{format}</span>
    <button
      type="button"
      class="bar-btn"
      disabled={disabled || busy === 'record-dir'}
      onclick={onChangeFolder}
    >
      <Icon name="folder" size={13} />
      {busy === 'record-dir' ? 'Choosing…' : 'Change folder…'}
    </button>
    <button
      type="button"
      class="bar-btn primary"
      disabled={disabled || busy === 'record-dir' || !directory}
      onclick={onOpenFolder}
    >
      <Icon name="external" size={13} />
      Open folder
    </button>
  </div>
</div>

<style>
  .recording-bar {
    flex: none;
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 6px 10px;
    border-radius: 9px;
    border: 1px solid var(--border-soft);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.018), transparent), var(--surface-1);
    box-shadow: var(--shadow-panel);
  }

  .recording-bar.live {
    border-color: rgba(255, 105, 105, 0.25);
    background:
      linear-gradient(90deg, rgba(255, 84, 84, 0.055), transparent 34%),
      var(--surface-1);
  }

  .recording-info {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .recording-icon {
    flex: none;
    display: grid;
    place-items: center;
    width: 27px;
    height: 27px;
    border-radius: 7px;
    color: var(--text-faint);
    background: var(--surface-2);
    border: 1px solid var(--border-soft);
  }

  .recording-icon.active {
    color: #ff8585;
    border-color: rgba(255, 105, 105, 0.24);
    background: rgba(255, 84, 84, 0.09);
  }

  .recording-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .recording-label,
  .last-label {
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--text-faint);
  }

  .recording-target {
    display: inline-flex;
    margin-left: 5px;
    padding: 1px 4px;
    border-radius: 3px;
    background: rgba(255, 100, 100, 0.12);
    color: #ffadad;
    letter-spacing: 0.04em;
  }

  .recording-path {
    max-width: min(42vw, 520px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .last-file {
    flex: none;
    max-width: min(24vw, 290px);
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding-left: 11px;
    margin-left: 2px;
    border-left: 1px solid var(--border);
  }

  .last-file.empty {
    opacity: 0.62;
  }

  .last-name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 9px;
    color: var(--text-faint);
  }

  .recording-actions {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .format-chip {
    height: 22px;
    display: inline-flex;
    align-items: center;
    padding: 0 7px;
    border-radius: 5px;
    border: 1px solid var(--border-soft);
    background: var(--surface-0);
    color: var(--text-faint);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.03em;
    white-space: nowrap;
  }

  .bar-btn {
    height: 27px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 9px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 9px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition: 130ms ease;
  }

  .bar-btn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--border-strong);
    background: var(--surface-3);
  }

  .bar-btn.primary {
    color: var(--text);
    border-color: var(--border-strong);
  }

  .bar-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  @media (max-width: 1180px) {
    .last-file {
      display: none;
    }

    .recording-path {
      max-width: 26vw;
    }

    .format-chip {
      display: none;
    }
  }
</style>
