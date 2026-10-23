<script lang="ts">
  import AudioChannel from './AudioChannel.svelte'
  import Icon from './Icon.svelte'
  import type {
    DeviceLists,
    Layout,
    LayoutPreset,
    QuickResizeRequest,
    StudioSnapshot,
  } from '../types'

  let {
    snapshot,
    activeLayout,
    devices,
    devicesLoading = false,
    presets,
    audioMeters = {},
    busy = '',
    disabled = false,
    onMute,
    onMonitor,
    onVolume,
    onQuickResize,
    onBackground,
    onBackgroundLayer,
    onBackgroundImage,
    onCamera,
    onCameraConnected,
    onScreen,
    onScreenConnected,
    onVisibility,
    onRefreshDevices,
    onSavePreset,
    onLoadPreset,
  }: {
    snapshot: StudioSnapshot | null
    activeLayout: Layout | null
    devices: DeviceLists | null
    devicesLoading?: boolean
    presets: LayoutPreset[]
    audioMeters?: Record<string, number>
    busy?: string
    disabled?: boolean
    onMute: (input: string, muted: boolean) => void
    onMonitor: (input: string, monitoring: boolean) => void
    onVolume: (input: string, db: number) => void
    onQuickResize: (request: QuickResizeRequest) => void
    onBackground: (enabled: boolean) => void
    onBackgroundLayer: (
      mode: 'none' | 'color' | 'image',
      color: string,
      image?: string | null,
    ) => void
    onBackgroundImage: () => void
    onCamera: (value: string) => void
    onCameraConnected: (connected: boolean) => void
    onScreen: (mode: 'display' | 'window', value: string, name: string) => void
    onScreenConnected: (connected: boolean) => void
    onVisibility: (source: string, visible: boolean) => void
    onRefreshDevices: () => void
    onSavePreset: (name: string) => void
    onLoadPreset: (name: string) => void
  } = $props()

  let presetName = $state('')
  let sizeValue = $state(25)
  let splitValue = $state(71)
  let selectedCorner = $state('br')
  let resizeTimer: number | undefined
  let pendingResize: QuickResizeRequest | null = null
  let lastResizeAt = 0
  let shape = $derived((snapshot?.camera_radius ?? 0) >= 400 ? 'circle' : 'tile')
  let cameraValue = $derived(snapshot?.camera?.value ?? '')
  let screenChoice = $derived(
    snapshot?.screen_mode === 'window'
      ? snapshot.window
        ? `window|${snapshot.window.value}`
        : ''
      : snapshot?.screen
        ? `display|${snapshot.screen.value}`
        : '',
  )
  let isSideBySide = $derived(
    activeLayout?.id === 'side_by_side' || activeLayout?.id === 'side_by_side_inv',
  )
  let cameraConnected = $derived(snapshot?.camera_connected ?? false)
  let screenConnected = $derived(snapshot?.screen_connected ?? false)
  let cameraName = $derived(snapshot?.camera?.name ?? 'Camera')
  let screenName = $derived(
    snapshot?.screen_mode === 'window'
      ? (snapshot?.window?.name ?? 'Application window')
      : (snapshot?.screen?.name ?? 'Display'),
  )
  let screenSourceName = $derived(
    snapshot?.screen_mode === 'window' ? 'Vidyst_Window' : 'Vidyst_Screen',
  )
  let backgroundMode = $derived(snapshot?.background_mode ?? 'none')
  let backgroundColor = $derived(snapshot?.background_color ?? '#0b0d12')
  let backgroundImageName = $derived(
    snapshot?.background_image
      ? (snapshot.background_image.split(/[\\/]/).pop() ?? 'Background image')
      : 'Choose an image…',
  )

  function toggleBackgroundImage() {
    if (backgroundMode === 'image') {
      onBackgroundLayer('none', backgroundColor)
    } else if (snapshot?.background_image) {
      onBackgroundLayer('image', backgroundColor, snapshot.background_image)
    } else {
      onBackgroundImage()
    }
  }

  function savePreset() {
    const name = presetName.trim()
    if (!name) return
    onSavePreset(name)
    presetName = ''
  }

  function flushResize() {
    if (resizeTimer !== undefined) {
      window.clearTimeout(resizeTimer)
      resizeTimer = undefined
    }
    if (!pendingResize) return
    lastResizeAt = performance.now()
    onQuickResize(pendingResize)
    pendingResize = null
  }

  function queueResize(request: QuickResizeRequest) {
    pendingResize = request
    const wait = Math.max(0, 33 - (performance.now() - lastResizeAt))
    if (wait === 0) {
      flushResize()
    } else if (resizeTimer === undefined) {
      resizeTimer = window.setTimeout(flushResize, wait)
    }
  }

  function finalizeResize(request: QuickResizeRequest) {
    if (resizeTimer !== undefined) {
      window.clearTimeout(resizeTimer)
      resizeTimer = undefined
    }
    pendingResize = null
    lastResizeAt = performance.now()
    onQuickResize(request)
  }

  function sizeInput(event: Event) {
    sizeValue = Number((event.currentTarget as HTMLInputElement).value)
    queueResize({ size_pct: sizeValue })
  }

  function sizeChanged(event: Event) {
    sizeValue = Number((event.currentTarget as HTMLInputElement).value)
    finalizeResize({ size_pct: sizeValue })
  }

  function splitInput(event: Event) {
    splitValue = Number((event.currentTarget as HTMLInputElement).value)
    queueResize({ split_pct: splitValue })
  }

  function splitChanged(event: Event) {
    splitValue = Number((event.currentTarget as HTMLInputElement).value)
    finalizeResize({ split_pct: splitValue })
  }

  function snapTo(corner: string) {
    selectedCorner = corner
    onQuickResize({ corner })
  }

  function screenChanged(event: Event) {
    const choice = (event.currentTarget as HTMLSelectElement).value
    const separator = choice.indexOf('|')
    if (separator < 0) return
    const mode = choice.slice(0, separator) as 'display' | 'window'
    const value = choice.slice(separator + 1)
    const name =
      mode === 'window'
        ? (devices?.windows.find((item) => item.value === value)?.name ?? 'Application window')
        : (devices?.screens.find((item) => item.value === value)?.name ?? 'Display')
    onScreen(mode, value, name)
  }
</script>

<aside class="rail">
  <div class="rail-head">
    <div>
      <span class="eyebrow">Control room</span>
      <h2>{activeLayout?.name ?? 'Standby'}</h2>
    </div>
    <div class="rail-status">
      <span class="connection-dot" class:online={snapshot?.connected}></span>
      <span>{snapshot?.connected ? 'OBS online' : 'OBS offline'}</span>
    </div>
  </div>

  <div class="rail-scroll">
    <section class="section">
      <header class="section-head">
        <span class="section-title">Sources</span>
        <button
          type="button"
          class="text-btn"
          onclick={onRefreshDevices}
          disabled={disabled || devicesLoading}
        >
          <Icon name="refresh" size={12} />
          {devicesLoading ? 'Scanning…' : 'Refresh'}
        </button>
      </header>

      <div class="source-row">
        <div class="source-icon"><Icon name="camera" size={15} /></div>
        <div class="source-copy">
          <span class="source-label">Camera</span>
          <span class="source-value" title={cameraName}>{cameraName}</span>
        </div>
        <button
          type="button"
          class="visibility"
          class:on={snapshot?.camera_visible}
          title={snapshot?.camera_visible ? 'Hide camera' : 'Show camera'}
          onclick={() => onVisibility('Vidyst_Camera_Group', !snapshot?.camera_visible)}
          disabled={disabled}
        >
          <Icon name={snapshot?.camera_visible ? 'eye' : 'eye-off'} size={14} />
        </button>
      </div>
      <select
        class="device-select"
        value={cameraValue}
        onchange={(event) => onCamera(event.currentTarget.value)}
        disabled={disabled || devicesLoading}
        aria-label="Camera device"
      >
        {#if !devices?.cameras.some((item) => item.value === cameraValue)}
          <option value={cameraValue}>{cameraName}</option>
        {/if}
        {#each devices?.cameras ?? [] as device (device.value)}
          <option value={device.value}>{device.name}</option>
        {/each}
      </select>
      <div class="source-connection">
        <span class:online={cameraConnected}>
          {cameraConnected ? 'Camera active' : 'Camera released'}
        </span>
        <button
          type="button"
          class="connect-btn"
          class:on={cameraConnected}
          title={cameraConnected ? 'Release the camera device' : 'Connect the selected camera'}
          onclick={() => onCameraConnected(!cameraConnected)}
          disabled={disabled || busy === 'camera-connection'}
        >
          <Icon name="power" size={12} />
          {busy === 'camera-connection' ? 'Working…' : cameraConnected ? 'Disconnect' : 'Connect'}
        </button>
      </div>

      <div class="source-row screen-row">
        <div class="source-icon"><Icon name="screen" size={15} /></div>
        <div class="source-copy">
          <span class="source-label">Screen / Window</span>
          <span class="source-value" title={screenName}>{screenName}</span>
        </div>
        <button
          type="button"
          class="visibility"
          class:on={snapshot?.screen_visible}
          title={snapshot?.screen_visible ? 'Hide screen' : 'Show screen'}
          onclick={() => onVisibility(screenSourceName, !snapshot?.screen_visible)}
          disabled={disabled}
        >
          <Icon name={snapshot?.screen_visible ? 'eye' : 'eye-off'} size={14} />
        </button>
      </div>
      <select
        class="device-select"
        value={screenChoice}
        onchange={screenChanged}
        disabled={disabled || devicesLoading}
        aria-label="Screen or window source"
      >
        {#if screenChoice && !devices?.screens.some((item) => `display|${item.value}` === screenChoice) && !devices?.windows.some((item) => `window|${item.value}` === screenChoice)}
          <option value={screenChoice}>{screenName}</option>
        {/if}
        {#if (devices?.screens.length ?? 0) > 0}
          <optgroup label="Displays">
            {#each devices?.screens ?? [] as device (device.value)}
              <option value={`display|${device.value}`}>{device.name}</option>
            {/each}
          </optgroup>
        {/if}
        {#if (devices?.windows.length ?? 0) > 0}
          <optgroup label="Application windows">
            {#each devices?.windows ?? [] as device (device.value)}
              <option value={`window|${device.value}`}>{device.name}</option>
            {/each}
          </optgroup>
        {/if}
      </select>
      <div class="source-connection">
        <span class:online={screenConnected}>
          {screenConnected ? 'Screen active' : 'Screen released'}
        </span>
        <button
          type="button"
          class="connect-btn"
          class:on={screenConnected}
          title={screenConnected ? 'Release the screen capture' : 'Connect the selected screen or window'}
          onclick={() => onScreenConnected(!screenConnected)}
          disabled={disabled || busy === 'screen-connection'}
        >
          <Icon name="power" size={12} />
          {busy === 'screen-connection' ? 'Working…' : screenConnected ? 'Disconnect' : 'Connect'}
        </button>
      </div>
    </section>

    <section class="section" id="audio-mixer">
      <header class="section-head">
        <span class="section-title">Audio mixer</span>
        <span class="section-meta">2 channels</span>
      </header>
      <div class="audio-list">
        {#each snapshot?.audio ?? [] as channel (channel.input)}
          <AudioChannel
            {channel}
            {disabled}
            meterDb={audioMeters[channel.input] ?? -60}
            onMute={(muted) => onMute(channel.input, muted)}
            onMonitor={(monitoring) => onMonitor(channel.input, monitoring)}
            onVolume={(db) => onVolume(channel.input, db)}
          />
        {/each}
      </div>
    </section>

    <section class="section" id="quick-customize">
      <header class="section-head">
        <span class="section-title">Quick composition</span>
        <span class="section-meta">{shape === 'circle' ? 'Circle' : 'Rounded tile'}</span>
      </header>

      <div class="field">
        <span class="field-label">Camera shape</span>
        <div class="segmented">
          <button
            type="button"
            class:active={shape === 'circle'}
            onclick={() => onQuickResize({ shape: 'circle' })}
            disabled={disabled || busy === 'resize'}
          >
            <Icon name="circle" size={13} /> Circle
          </button>
          <button
            type="button"
            class:active={shape === 'tile'}
            onclick={() => onQuickResize({ shape: 'tile' })}
            disabled={disabled || busy === 'resize'}
          >
            <Icon name="square" size={13} /> Tile
          </button>
        </div>
      </div>

      <div class="field">
        <div class="field-line">
          <span class="field-label">Size</span>
          <span class="field-value">{sizeValue}%</span>
        </div>
        <input
          class="slider"
          type="range"
          min="8"
          max="75"
          step="1"
          value={sizeValue}
          disabled={disabled || busy === 'resize'}
          oninput={sizeInput}
          onchange={sizeChanged}
          aria-label="Camera size"
        />
      </div>

      <div class="field">
        <span class="field-label">Snap to corner</span>
        <div class="corner-grid">
          {#each [['tl', 'Top left'], ['tr', 'Top right'], ['bl', 'Bottom left'], ['br', 'Bottom right']] as [corner, label] (corner)}
            <button
              type="button"
              title={label}
              onclick={() => snapTo(corner)}
              disabled={disabled || busy === 'resize'}
            >
              <span class="corner-glyph" class:active-corner={corner === selectedCorner}></span>
              {label}
            </button>
          {/each}
        </div>
      </div>

      {#if isSideBySide}
        <div class="field">
          <div class="field-line">
            <span class="field-label">Split</span>
            <span class="field-value">{splitValue} / {100 - splitValue}</span>
          </div>
          <input
            class="slider"
            type="range"
            min="35"
            max="85"
            step="1"
            value={splitValue}
            disabled={disabled || busy === 'resize'}
            oninput={splitInput}
            onchange={splitChanged}
            aria-label="Side by side split"
          />
        </div>
      {/if}

      <div class="toggle-row">
        <div>
          <span class="field-label">Background removal</span>
          <span class="field-help">Uses the OBS background removal filter</span>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            checked={snapshot?.background_removal ?? false}
            disabled={disabled || busy === 'background'}
            onchange={(event) => onBackground(event.currentTarget.checked)}
          />
          <span class="switch-track"></span>
        </label>
      </div>

      <div class="field background-layer">
        <div class="field-line">
          <span class="field-label">Background layer</span>
          <span class="field-value">{backgroundMode === 'color'
            ? 'Color'
            : backgroundMode === 'image'
              ? 'Image'
              : 'Off'}</span>
        </div>
        <div class="color-row">
          <input
            class="color-input"
            type="color"
            value={backgroundColor}
            aria-label="Background color"
            disabled={disabled || busy === 'background-layer'}
            onchange={(event) =>
              onBackgroundLayer('color', event.currentTarget.value)}
          />
          <button
            type="button"
            class="color-toggle"
            class:active={backgroundMode === 'color'}
            disabled={disabled || busy === 'background-layer'}
            onclick={() =>
              onBackgroundLayer(backgroundMode === 'color' ? 'none' : 'color', backgroundColor)}
          >
            {backgroundMode === 'color' ? 'Hide color' : 'Show color'}
          </button>
        </div>
        <div class="image-row">
          <button
            type="button"
            class="image-pick"
            title={snapshot?.background_image ?? 'Choose a background image'}
            disabled={disabled || busy === 'background-layer'}
            onclick={onBackgroundImage}
          >
            <Icon name="image" size={13} />
            <span>{backgroundImageName}</span>
          </button>
          <button
            type="button"
            class="color-toggle image-toggle"
            class:active={backgroundMode === 'image'}
            disabled={disabled || busy === 'background-layer'}
            onclick={toggleBackgroundImage}
          >
            {backgroundMode === 'image' ? 'Hide image' : 'Show image'}
          </button>
        </div>
        <span class="field-help">Solid color or a local PNG/JPG behind the camera</span>
      </div>
    </section>

    <section class="section presets">
      <header class="section-head">
        <span class="section-title">Presets</span>
        <span class="section-meta">{presets.length} saved</span>
      </header>
      <div class="preset-save">
        <input
          type="text"
          placeholder="Name this layout…"
          maxlength="64"
          bind:value={presetName}
          onkeydown={(event) => event.key === 'Enter' && savePreset()}
          disabled={disabled}
        />
        <button
          type="button"
          class="save-btn"
          onclick={savePreset}
          disabled={disabled || !presetName.trim() || busy === 'preset'}
        >
          Save
        </button>
      </div>
      <div class="preset-list">
        {#each presets as preset (preset.name)}
          <button
            type="button"
            class="preset"
            onclick={() => onLoadPreset(preset.name)}
            disabled={disabled || busy === 'preset'}
          >
            <span class="preset-icon"><Icon name="sparkles" size={12} /></span>
            <span class="preset-name">{preset.name}</span>
            <span class="preset-layout">{preset.layout_id.replaceAll('_', ' ')}</span>
          </button>
        {:else}
          <p class="empty-hint">Save the current composition to recall it in one click.</p>
        {/each}
      </div>
    </section>
  </div>
</aside>

<style>
  .rail {
    flex: none;
    width: 336px;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-lg);
    background: var(--surface-1);
    border: 1px solid var(--border-soft);
    box-shadow: var(--shadow-panel);
    overflow: hidden;
  }

  .rail-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 15px 15px 12px;
    border-bottom: 1px solid var(--border-soft);
  }

  .eyebrow {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .rail-head h2 {
    margin: 5px 0 0;
    font-size: 17px;
    line-height: 1.15;
    font-weight: 650;
    letter-spacing: -0.02em;
    color: var(--text);
  }

  .rail-status {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 7px;
    border-radius: 999px;
    background: var(--surface-2);
    border: 1px solid var(--border-soft);
    color: var(--text-faint);
    font-size: 9px;
    white-space: nowrap;
  }

  .connection-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--text-faint);
  }

  .connection-dot.online {
    background: var(--success);
    box-shadow: 0 0 0 2px rgba(54, 211, 153, 0.13);
  }

  .rail-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 15px 16px;
    scrollbar-width: thin;
    scrollbar-color: #323a48 transparent;
  }

  .section {
    padding: 13px 0;
    border-bottom: 1px solid var(--border-soft);
  }

  .section:last-child {
    border-bottom: 0;
  }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 9px;
  }

  .section-title {
    font-size: 11px;
    font-weight: 650;
    color: var(--text);
    letter-spacing: -0.005em;
  }

  .section-meta {
    font-size: 9px;
    color: var(--text-faint);
  }

  .text-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font-size: 9px;
    cursor: pointer;
    padding: 0;
  }

  .text-btn:hover {
    color: var(--text-muted);
  }

  .source-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 3px;
    padding: 6px 7px;
    border-radius: 8px;
    background: var(--surface-2);
    border: 1px solid var(--border-soft);
  }

  .source-row.screen-row {
    margin-top: 8px;
  }

  .source-icon {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    background: rgba(132, 112, 255, 0.08);
    color: #a69af8;
    flex: none;
  }

  .source-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
  }

  .source-label {
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .source-value {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .visibility {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }

  .visibility.on {
    color: #a9d1ff;
    background: rgba(95, 151, 255, 0.1);
  }

  .visibility:hover {
    border-color: var(--border-soft);
    background: var(--surface-3);
  }

  .source-connection {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 3px 2px 0;
    font-size: 9px;
    color: var(--text-faint);
  }

  .source-connection span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .source-connection span::before {
    content: '';
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--text-faint);
    opacity: 0.7;
  }

  .source-connection span.online {
    color: #8fd6b0;
  }

  .source-connection span.online::before {
    background: #57d38f;
    box-shadow: 0 0 6px rgba(87, 211, 143, 0.55);
  }

  .connect-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 7px;
    border-radius: 6px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 9px;
    cursor: pointer;
  }

  .connect-btn.on {
    color: #f2a2a2;
    border-color: rgba(242, 107, 107, 0.22);
    background: rgba(242, 107, 107, 0.08);
  }

  .connect-btn:hover:not(:disabled) {
    border-color: rgba(150, 130, 255, 0.4);
    color: var(--text);
  }

  .connect-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .device-select {
    width: 100%;
    height: 29px;
    margin-top: 4px;
    padding: 0 26px 0 8px;
    border-radius: 7px;
    border: 1px solid var(--border-soft);
    background:
      linear-gradient(45deg, transparent 50%, var(--text-faint) 50%) calc(100% - 13px) 12px / 4px 4px no-repeat,
      linear-gradient(135deg, var(--text-faint) 50%, transparent 50%) calc(100% - 9px) 12px / 4px 4px no-repeat,
      var(--surface-2);
    color: var(--text-muted);
    font-size: 10px;
    appearance: none;
    cursor: pointer;
    outline: none;
  }

  .device-select:focus {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  }

  .audio-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 10px;
  }

  .field-label {
    display: block;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.035em;
    color: var(--text-muted);
  }

  .field-help {
    display: block;
    margin-top: 2px;
    font-size: 8px;
    color: var(--text-faint);
  }

  .field-line {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .field-value {
    font-size: 9px;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }

  .background-layer {
    margin-top: 7px;
    padding-top: 9px;
    border-top: 1px solid var(--border-soft);
  }

  .color-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .color-input {
    width: 42px;
    height: 27px;
    padding: 2px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-0);
    cursor: pointer;
  }

  .image-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .image-pick {
    flex: 1;
    min-width: 0;
    height: 27px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-0);
    color: var(--text-muted);
    font-size: 9px;
    cursor: pointer;
    transition: 130ms ease;
  }

  .image-pick span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .image-pick:hover {
    color: var(--text);
    border-color: var(--border-strong);
  }

  .image-toggle {
    flex: none;
    width: 92px;
  }

  .color-input::-webkit-color-swatch-wrapper {
    padding: 0;
  }

  .color-input::-webkit-color-swatch {
    border: 0;
    border-radius: 3px;
  }

  .color-toggle {
    flex: 1;
    height: 27px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-0);
    color: var(--text-muted);
    font-size: 9.5px;
    font-weight: 600;
    cursor: pointer;
    transition: 130ms ease;
  }

  .color-toggle:hover {
    color: var(--text);
    border-color: var(--border-strong);
  }

  .color-toggle.active {
    color: var(--text);
    border-color: var(--border-strong);
    background: var(--surface-3);
  }

  .segmented {
    display: flex;
    padding: 2px;
    border-radius: 8px;
    background: var(--surface-0);
    border: 1px solid var(--border-soft);
  }

  .segmented button {
    flex: 1;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-faint);
    font-size: 9.5px;
    font-weight: 600;
    cursor: pointer;
    transition: 130ms ease;
  }

  .segmented button:hover {
    color: var(--text-muted);
  }

  .segmented button.active {
    background: var(--surface-3);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
  }

  .slider {
    width: 100%;
    height: 16px;
    margin: 0;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }

  .slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--accent) 0 34%, #2b3442 34% 100%);
  }

  .slider::-webkit-slider-thumb {
    appearance: none;
    width: 11px;
    height: 11px;
    margin-top: -3.5px;
    border-radius: 50%;
    background: #f2f1f8;
    border: 1px solid rgba(0, 0, 0, 0.4);
  }

  .corner-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
  }

  .corner-grid button {
    height: 27px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 7px;
    border-radius: 6px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
    color: var(--text-faint);
    font-size: 8.5px;
    cursor: pointer;
    transition: 120ms ease;
  }

  .corner-grid button:hover {
    border-color: var(--border-strong);
    color: var(--text-muted);
  }

  .corner-glyph {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    border: 1.5px solid currentColor;
    position: relative;
    flex: none;
  }

  .corner-glyph::after {
    content: '';
    position: absolute;
    width: 3px;
    height: 3px;
    right: -1px;
    bottom: -1px;
    border-radius: 1px;
    background: currentColor;
  }

  .corner-glyph.active-corner {
    color: var(--accent);
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding-top: 2px;
  }

  .switch {
    position: relative;
    display: inline-flex;
    flex: none;
    width: 32px;
    height: 18px;
    cursor: pointer;
  }

  .switch input {
    position: absolute;
    opacity: 0;
    inset: 0;
    cursor: pointer;
  }

  .switch-track {
    width: 100%;
    height: 100%;
    border-radius: 999px;
    background: #29303b;
    border: 1px solid var(--border);
    transition: 160ms ease;
  }

  .switch-track::after {
    content: '';
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #8e96a5;
    transition: 160ms ease;
  }

  .switch input:checked + .switch-track {
    background: color-mix(in srgb, var(--accent) 28%, #181a24);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  }

  .switch input:checked + .switch-track::after {
    left: 17px;
    background: var(--accent);
    box-shadow: 0 0 8px color-mix(in srgb, var(--accent) 55%, transparent);
  }

  .preset-save {
    display: flex;
    gap: 5px;
  }

  .preset-save input {
    flex: 1;
    min-width: 0;
    height: 29px;
    padding: 0 8px;
    border-radius: 7px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
    color: var(--text);
    font-size: 10px;
    outline: none;
  }

  .preset-save input:focus {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  }

  .preset-save input::placeholder {
    color: var(--text-faint);
  }

  .save-btn {
    height: 29px;
    padding: 0 10px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--surface-3);
    color: var(--text-muted);
    font-size: 9px;
    font-weight: 600;
    cursor: pointer;
  }

  .save-btn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--border-strong);
  }

  .save-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .preset-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 6px;
  }

  .preset {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    height: 30px;
    padding: 0 7px;
    border-radius: 7px;
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    text-align: left;
    transition: 120ms ease;
  }

  .preset:hover {
    background: var(--surface-2);
    border-color: var(--border);
  }

  .preset-icon {
    display: grid;
    place-items: center;
    color: var(--accent);
    flex: none;
  }

  .preset-name {
    font-size: 9.5px;
    font-weight: 600;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .preset-layout {
    margin-left: auto;
    font-size: 8px;
    color: var(--text-faint);
    white-space: nowrap;
    text-transform: capitalize;
  }

  .empty-hint {
    margin: 3px 0 0;
    font-size: 9px;
    line-height: 1.45;
    color: var(--text-faint);
  }

  @media (max-width: 1240px) {
    .rail {
      width: 304px;
    }
  }

  @media (max-height: 760px) {
    .rail-head {
      padding: 11px 13px 9px;
    }

    .section {
      padding: 10px 0;
    }
  }
</style>
