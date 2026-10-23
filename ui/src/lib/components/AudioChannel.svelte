<script lang="ts">
  import Icon from './Icon.svelte'
  import type { AudioChannel } from '../types'

  let {
    channel,
    onMute,
    onMonitor,
    onVolume,
    meterDb = -60,
    disabled = false,
  }: {
    channel: AudioChannel
    onMute: (muted: boolean) => void
    onMonitor: (monitoring: boolean) => void
    onVolume: (db: number) => void
    meterDb?: number
    disabled?: boolean
  } = $props()

  let localVolume = $state<number>(0)
  let percent = $derived(Math.max(0, Math.min(100, ((localVolume + 60) / 70) * 100)))
  let meterPercent = $derived(
    Math.max(0, Math.min(100, ((meterDb + 60) / 60) * 100)),
  )
  let meterBars = $derived(
    Array.from({ length: 28 }, (_, index) => ({
      on: meterPercent >= ((index + 1) / 28) * 100,
      tone: index < 19 ? 'green' : index < 24 ? 'yellow' : 'red',
    })),
  )

  $effect(() => {
    localVolume = channel.volume_db
  })

  function commit() {
    onVolume(localVolume)
  }

  function reset() {
    localVolume = 0
    onVolume(0)
  }
</script>

<div class="channel" class:muted={channel.muted} class:disabled>
  <div class="channel-top">
    <div class="channel-icon">
      <Icon name={channel.input.includes('Screen') ? 'desktop' : 'mic'} size={15} />
    </div>
    <span class="channel-label">{channel.label}</span>
    <button
      type="button"
      class="channel-value"
      title="Double-click to reset"
      aria-label="Reset {channel.label} volume to 0 dB"
      ondblclick={reset}
    >
      {localVolume <= -59.5 ? '−∞' : `${localVolume.toFixed(0)} dB`}
    </button>
  </div>

  <div class="vu-row" title={`Live input level: ${meterDb <= -59.5 ? '−∞' : `${meterDb.toFixed(1)} dB`}`}>
    <span class="vu-label">VU</span>
    <div
      class="vu"
      class:silent={meterDb <= -59.5}
      role="meter"
      aria-label={`${channel.label} live level`}
      aria-valuemin={-60}
      aria-valuemax={0}
      aria-valuenow={Number(meterDb.toFixed(1))}
    >
      {#each meterBars as bar, index (index)}
        <span class="vu-seg" class:on={bar.on} class:green={bar.tone === 'green'} class:yellow={bar.tone === 'yellow'} class:red={bar.tone === 'red'}></span>
      {/each}
    </div>
    <span class="vu-db">{meterDb <= -59.5 ? '−∞' : meterDb.toFixed(0)}</span>
  </div>

  <div class="meter-row">
    <input
      class="volume"
      type="range"
      min="-60"
      max="10"
      step="1"
      value={localVolume}
      style:--fill={`${percent}%`}
      aria-label={`${channel.label} volume`}
      oninput={(event) => (localVolume = Number(event.currentTarget.value))}
      onchange={commit}
      {disabled}
    />
    <button
      type="button"
      class="mini-btn"
      class:monitoring={channel.monitoring}
      title={channel.monitoring ? 'Stop monitoring' : 'Monitor in headphones'}
      onclick={() => onMonitor(!channel.monitoring)}
      {disabled}
    >
      <Icon name="headphones" size={13} />
    </button>
    <button
      type="button"
      class="mini-btn mute"
      class:muted={channel.muted}
      title={channel.muted ? 'Unmute' : 'Mute'}
      onclick={() => onMute(!channel.muted)}
      {disabled}
    >
      <Icon name={channel.muted ? 'eye-off' : 'volume'} size={13} />
    </button>
  </div>
</div>

<style>
  .channel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 9px 7px;
    border-radius: 9px;
    background: var(--surface-2);
    border: 1px solid var(--border-soft);
    transition: opacity 160ms ease;
  }

  .channel.muted {
    opacity: 0.62;
  }

  .channel.disabled {
    pointer-events: none;
    opacity: 0.45;
  }

  .channel-top {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .channel-icon {
    display: grid;
    place-items: center;
    color: var(--text-faint);
    flex: none;
  }

  .channel-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .channel-value {
    margin-left: auto;
    padding: 0;
    border: 0;
    background: transparent;
    font-size: 9px;
    font-variant-numeric: tabular-nums;
    color: var(--text-faint);
    cursor: default;
    flex: none;
  }

  .vu-row {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 9px;
  }

  .vu-label {
    flex: none;
    font-size: 7px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--text-faint);
  }

  .vu {
    flex: 1;
    display: flex;
    align-items: stretch;
    gap: 1px;
    height: 7px;
    padding: 1px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.32);
    border: 1px solid rgba(255, 255, 255, 0.04);
    overflow: hidden;
  }

  .vu-seg {
    flex: 1;
    min-width: 1px;
    border-radius: 1px;
    background: rgba(255, 255, 255, 0.055);
    transition: background 55ms linear;
  }

  .vu-seg.on.green {
    background: #42d392;
    box-shadow: 0 0 4px rgba(66, 211, 146, 0.35);
  }

  .vu-seg.on.yellow {
    background: #f0c24f;
    box-shadow: 0 0 4px rgba(240, 194, 79, 0.4);
  }

  .vu-seg.on.red {
    background: #ff6464;
    box-shadow: 0 0 5px rgba(255, 100, 100, 0.5);
  }

  .vu-db {
    flex: none;
    width: 22px;
    text-align: right;
    font-size: 7.5px;
    font-variant-numeric: tabular-nums;
    color: var(--text-faint);
  }

  .meter-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .volume {
    --track: color-mix(in srgb, var(--text-faint) 20%, transparent);
    --filled: var(--accent);
    flex: 1;
    min-width: 0;
    height: 16px;
    margin: 0;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }

  .volume::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--filled) 0,
      var(--filled) var(--fill),
      var(--track) var(--fill),
      var(--track) 100%
    );
  }

  .volume::-webkit-slider-thumb {
    appearance: none;
    width: 11px;
    height: 11px;
    margin-top: -3.5px;
    border-radius: 50%;
    background: #f3f2fa;
    border: 1px solid rgba(0, 0, 0, 0.35);
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.4);
  }

  .volume:focus-visible {
    outline: none;
  }

  .volume:focus-visible::-webkit-slider-thumb {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .mini-btn {
    width: 23px;
    height: 23px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 6px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    transition: 120ms ease;
  }

  .mini-btn:hover {
    color: var(--text);
    background: var(--surface-3);
    border-color: var(--border-soft);
  }

  .mini-btn.monitoring {
    color: #8ab7ff;
    background: rgba(88, 139, 255, 0.12);
    border-color: rgba(104, 151, 255, 0.22);
  }

  .mini-btn.muted {
    color: #ff8585;
    background: rgba(255, 94, 94, 0.1);
    border-color: rgba(255, 108, 108, 0.2);
  }
</style>
