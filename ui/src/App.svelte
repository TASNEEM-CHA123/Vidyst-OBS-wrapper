<script lang="ts">
  import { onMount } from 'svelte'
  import { api, isTauri, onStudioEvent } from './lib/api'
  import { mockLayouts } from './lib/mock'
  import LayoutDock from './lib/components/LayoutDock.svelte'
  import PreviewStage from './lib/components/PreviewStage.svelte'
  import RecordingBar from './lib/components/RecordingBar.svelte'
  import RightRail from './lib/components/RightRail.svelte'
  import SettingsModal from './lib/components/SettingsModal.svelte'
  import Toasts from './lib/components/Toasts.svelte'
  import TopBar from './lib/components/TopBar.svelte'
  import Icon from './lib/components/Icon.svelte'
  import type {
    AspectTargets,
    AudioMetersPayload,
    ConnectionStatus,
    DeviceLists,
    Layout,
    LayoutPreset,
    PreviewFrame,
    QuickResizeRequest,
    SettingsView,
    StudioEvent,
    StudioSnapshot,
    Toast,
  } from './lib/types'

  let status = $state<ConnectionStatus>({
    state: 'connecting',
    message: 'Connecting to OBS…',
    connected: false,
    obs_version: null,
    websocket_version: null,
    rpc_version: null,
    reconnect_attempt: 0,
    bootstrap_ready: false,
  })
  let snapshot = $state<StudioSnapshot | null>(null)
  let layouts = $state<Layout[]>([])
  let settings = $state<SettingsView | null>(null)
  let devices = $state<DeviceLists | null>(null)
  let presets = $state<LayoutPreset[]>([])
  let aspects = $state<AspectTargets>({ horizontal: true, vertical: true })
  let toasts = $state<Toast[]>([])
  let busy = $state('')
  let settingsOpen = $state(false)
  let devicesLoading = $state(false)
  let initializing = $state(true)
  let horizontalFrame = $state<string | null>(null)
  let verticalFrame = $state<string | null>(null)
  let verticalPreviewError = $state<string | null>(null)
  let audioMeters = $state<Record<string, number>>({})
  let pageVisible = $state(true)

  let unlisteners: Array<() => void> = []
  let toastSequence = 0
  let refreshTimer: number | undefined
  let refreshing = false
  let refreshQueued = false
  let disposed = false

  let activeLayout = $derived(
    layouts.find((layout) => layout.id === snapshot?.active_layout) ?? null,
  )
  let previewLayout = $derived(
    activeLayout ?? layouts.find((layout) => layout.id === 'just_me') ?? null,
  )
  let previewLive = $derived(activeLayout !== null)
  let controlsDisabled = $derived(!status.connected)
  let activeLayoutName = $derived(
    snapshot?.active_layout_name ?? activeLayout?.name ?? null,
  )
  let outputActive = $derived(Boolean(snapshot?.recording || snapshot?.streaming))

  // OBS sends state changes rather than a clock, so poll while REC/LIVE is
  // active to keep the timecode and duration fields moving.
  $effect(() => {
    if (!outputActive) return
    const timer = window.setInterval(() => void refresh(), 1000)
    return () => window.clearInterval(timer)
  })

  // Only ask Rust for live frames while this window is visible and the
  // selected canvases need them.
  $effect(() => {
    if (!isTauri) return
    const active = status.connected && pageVisible
    void api
      .setPreviewActive(active, aspects.horizontal, aspects.vertical)
      .catch(() => {})
    if (!active) {
      horizontalFrame = null
      verticalFrame = null
    }
  })

  onMount(() => {
    unlisteners = []
    const onVisibility = () => {
      pageVisible = !document.hidden
    }
    document.addEventListener('visibilitychange', onVisibility)
    void start()
    return () => {
      disposed = true
      document.removeEventListener('visibilitychange', onVisibility)
      if (isTauri) void api.setPreviewActive(false, false, false).catch(() => {})
      if (refreshTimer !== undefined) window.clearTimeout(refreshTimer)
      for (const unlisten of unlisteners) unlisten()
      unlisteners = []
    }
  })

  async function start() {
    await bindEvents()
    try {
      const [nextStatus, nextLayouts, nextSnapshot, nextSettings, nextPresets] =
        await Promise.all([
          api.status(),
          api.layouts(),
          api.studio().catch(() => null),
          api.settings().catch(() => null),
          api.presets().catch(() => []),
        ])
      if (disposed) return
      status = nextStatus
      layouts = nextLayouts.length ? nextLayouts : mockLayouts
      snapshot = nextSnapshot
      settings = nextSettings
      presets = nextPresets
      if (status.connected) {
        void refreshAux()
      }
    } catch (error) {
      notify('error', messageFor(error))
    } finally {
      if (!disposed) initializing = false
    }
  }

  async function bindEvents() {
    const statusListener = await onStudioEvent<ConnectionStatus>('obs-status', (next) => {
      const wasConnected = status.connected
      status = next
      if (!next.connected) {
        horizontalFrame = null
        verticalFrame = null
        verticalPreviewError = null
        audioMeters = {}
      }
      if (!next.connected && snapshot) {
        snapshot = { ...snapshot, connected: false, bootstrap_ready: next.bootstrap_ready }
      }
      if (next.connected && (!wasConnected || !next.bootstrap_ready)) {
        scheduleRefresh(80)
        if (!wasConnected) void refreshAux()
      }
    })
    if (disposed) {
      statusListener()
      return
    }
    unlisteners.push(statusListener)

    const frameListener = await onStudioEvent<PreviewFrame>('obs-preview-frame', (frame) => {
      // OBS may return either raw base64 or a complete data URL depending on
      // the websocket/plugin version. Normalise both forms.
      const base64 = frame.image.startsWith('data:')
        ? frame.image.slice(frame.image.indexOf(',') + 1)
        : frame.image
      const source = `data:image/jpeg;base64,${base64}`
      if (frame.target === 'vertical') {
        verticalFrame = source
        verticalPreviewError = null
      } else {
        horizontalFrame = source
      }
    })
    if (disposed) {
      frameListener()
      return
    }
    unlisteners.push(frameListener)

    const previewErrorListener = await onStudioEvent<{ target: string; message: string }>(
      'obs-preview-error',
      (event) => {
        if (event.target === 'vertical') verticalPreviewError = event.message
      },
    )
    if (disposed) {
      previewErrorListener()
      return
    }
    unlisteners.push(previewErrorListener)

    const meterListener = await onStudioEvent<AudioMetersPayload>('audio-meters', (payload) => {
      const next: Record<string, number> = {}
      for (const meter of payload.meters) next[meter.input] = meter.peak_db
      audioMeters = next
    })
    if (disposed) {
      meterListener()
      return
    }
    unlisteners.push(meterListener)

    const eventListener = await onStudioEvent<StudioEvent>('obs-event', (event) => {
      if (
        [
          'program_scene',
          'record',
          'record_file',
          'stream',
          'mute',
          'volume',
          'monitor',
          'visibility',
          'filter',
          'input_settings',
          'bootstrap',
          'transition',
          'transition_duration',
          'vertical_output',
          'source_connection',
        ].includes(event.kind)
      ) {
        scheduleRefresh(110)
      }
    })
    if (disposed) {
      eventListener()
      return
    }
    unlisteners.push(eventListener)
  }

  function scheduleRefresh(delay = 180) {
    if (refreshTimer !== undefined) window.clearTimeout(refreshTimer)
    refreshTimer = window.setTimeout(() => {
      refreshTimer = undefined
      void refresh()
    }, delay)
  }

  async function refresh() {
    if (!status.connected || disposed) return
    if (refreshing) {
      refreshQueued = true
      return
    }
    refreshing = true
    try {
      const next = await api.studio()
      if (!disposed) snapshot = next
    } catch {
      // Connection events drive the visible error state; a refresh can race a
      // reconnect, so avoid flooding the user with transient errors.
    } finally {
      refreshing = false
      if (refreshQueued && !disposed) {
        refreshQueued = false
        scheduleRefresh(40)
      }
    }
  }

  async function refreshAux() {
    const [nextSettings, nextPresets] = await Promise.all([
      api.settings().catch(() => null),
      api.presets().catch(() => []),
    ])
    if (disposed) return
    if (nextSettings) settings = nextSettings
    presets = nextPresets
    if (status.connected) void loadDevices()
  }

  async function loadDevices() {
    if (!status.connected || devicesLoading) return
    devicesLoading = true
    try {
      const next = await api.devices()
      if (!disposed) devices = next
    } catch (error) {
      if (!disposed) notify('error', `Could not list devices: ${messageFor(error)}`)
    } finally {
      if (!disposed) devicesLoading = false
    }
  }

  function notify(kind: Toast['kind'], message: string) {
    const id = ++toastSequence
    toasts = [...toasts, { id, kind, message }]
    window.setTimeout(() => dismissToast(id), kind === 'error' ? 6500 : 3600)
  }

  function dismissToast(id: number) {
    toasts = toasts.filter((toast) => toast.id !== id)
  }

  function messageFor(error: unknown): string {
    const text = error instanceof Error ? error.message : String(error)
    return text.replace(/^Error:\s*/i, '').replace(/\s+/g, ' ').trim()
  }

  async function perform<T>(
    label: string,
    work: () => Promise<T>,
    success?: string,
  ): Promise<T | undefined> {
    if (busy) return
    busy = label
    try {
      const result = await work()
      if (success) notify('success', success)
      return result
    } catch (error) {
      notify('error', messageFor(error))
      return undefined
    } finally {
      busy = ''
    }
  }

  function toggleAspect(target: 'horizontal' | 'vertical') {
    const next = { ...aspects, [target]: !aspects[target] }
    if (!next.horizontal && !next.vertical) {
      notify('info', 'Keep at least one output canvas selected.')
      return
    }
    aspects = next
  }

  async function selectLayout(id: string) {
    await perform('layout', async () => {
      const next = await api.applyLayout(id, aspects.horizontal, aspects.vertical)
      snapshot = next
    })
  }

  let resizeBusy = false
  let queuedResize: QuickResizeRequest | null = null

  async function quickResize(request: QuickResizeRequest) {
    // Slider drags can outpace the WebSocket round trip. Keep applying the
    // newest transform until the queue drains so the final value always wins.
    queuedResize = request
    if (resizeBusy) return
    resizeBusy = true
    if (!busy) busy = 'resize'
    try {
      while (queuedResize && !disposed) {
        const nextRequest = queuedResize
        queuedResize = null
        try {
          snapshot = await api.quickResize(nextRequest)
        } catch (error) {
          notify('error', messageFor(error))
          break
        }
      }
    } finally {
      resizeBusy = false
      if (busy === 'resize') busy = ''
    }
  }

  async function setBackground(enabled: boolean) {
    await perform('background', async () => {
      snapshot = await api.toggleBackgroundRemoval(enabled)
    })
  }

  async function setBackgroundLayer(
    mode: 'none' | 'color' | 'image',
    color: string,
    image: string | null = null,
  ) {
    await perform(
      'background-layer',
      async () => {
        snapshot = await api.setBackground(mode, color, image)
      },
      mode === 'color'
        ? 'Background color enabled.'
        : mode === 'image'
          ? 'Background image enabled.'
          : 'Background layer hidden.',
    )
  }

  async function chooseBackgroundImage() {
    const next = await perform(
      'background-layer',
      () => api.pickBackgroundImage(),
      undefined,
    )
    if (next) {
      snapshot = next
      notify('success', 'Background image enabled.')
    }
  }

  async function changeRecordFolder() {
    const next = await perform('record-dir', () => api.pickRecordFolder())
    if (next) {
      snapshot = next
      notify('success', `Recording folder set to ${next.recording_directory ?? 'the selected folder'}.`)
    }
  }

  async function openRecordFolder() {
    try {
      await api.openRecordFolder()
    } catch (error) {
      notify('error', messageFor(error))
    }
  }

  async function setVisibility(source: string, visible: boolean) {
    await perform('visibility', async () => {
      snapshot = await api.setVisibility(source, visible)
    })
  }

  async function setCameraSource(value: string) {
    await perform('device', async () => {
      snapshot = await api.setCameraSource(value)
    }, 'Camera source changed.')
  }

  async function setCameraConnected(connected: boolean) {
    await perform(
      'camera-connection',
      async () => {
        snapshot = await api.setCameraConnected(connected)
      },
      connected ? 'Camera connected.' : 'Camera disconnected and released.',
    )
  }

  async function setScreenConnected(connected: boolean) {
    await perform(
      'screen-connection',
      async () => {
        snapshot = await api.setScreenConnected(connected)
      },
      connected ? 'Screen capture connected.' : 'Screen capture disconnected.',
    )
  }

  async function setScreenSource(mode: 'display' | 'window', value: string, name: string) {
    await perform(
      'device',
      async () => {
        snapshot =
          mode === 'window'
            ? await api.setWindowSource(value, name)
            : await api.setScreenSource(value)
      },
      mode === 'window' ? 'Application window selected.' : 'Display source changed.',
    )
  }

  async function mute(input: string, muted: boolean) {
    try {
      await api.setMute(input, muted)
      scheduleRefresh(80)
    } catch (error) {
      notify('error', messageFor(error))
    }
  }

  async function monitor(input: string, monitoring: boolean) {
    try {
      await api.setMonitor(input, monitoring)
      if (monitoring) {
        notify('info', 'Monitoring enabled. Use headphones to avoid a feedback loop.')
      }
      scheduleRefresh(80)
    } catch (error) {
      notify('error', messageFor(error))
    }
  }

  async function volume(input: string, db: number) {
    try {
      await api.setVolume(input, db)
      scheduleRefresh(80)
    } catch (error) {
      notify('error', messageFor(error))
    }
  }

  async function toggleRecording() {
    await perform(
      'record',
      async () => {
        if (snapshot?.recording || snapshot?.recording_vertical) {
          await api.stopRecording()
          await new Promise((resolve) => window.setTimeout(resolve, 320))
          await refresh()
        } else {
          await api.startRecording(aspects.horizontal, aspects.vertical)
          await new Promise((resolve) => window.setTimeout(resolve, 320))
          await refresh()
        }
      },
      snapshot?.recording || snapshot?.recording_vertical ? 'Recording saved.' : 'Recording started.',
    )
  }

  async function toggleStreaming() {
    await perform(
      'live',
      async () => {
        if (snapshot?.streaming || snapshot?.streaming_vertical) {
          await api.stopStreaming()
          await new Promise((resolve) => window.setTimeout(resolve, 320))
          await refresh()
        } else {
          await api.startStreaming(aspects.horizontal, aspects.vertical)
          await new Promise((resolve) => window.setTimeout(resolve, 320))
          await refresh()
        }
      },
      snapshot?.streaming || snapshot?.streaming_vertical ? 'Stream ended.' : 'You are live.',
    )
  }

  async function splitRecording() {
    await perform('split', () => api.splitRecording(), 'Recording split.')
  }

  function scrollRail(section: string) {
    document.getElementById(section)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  async function openProjector(target: 'horizontal' | 'vertical') {
    try {
      await api.projector(target)
      notify('success', `${target === 'horizontal' ? 'Horizontal' : 'Vertical'} projector opened.`)
    } catch (error) {
      notify('error', messageFor(error))
    }
  }

  async function prepareScenes() {
    const next = await perform(
      'prepare',
      () => api.prepareScenes(),
      'Scenes prepared. Your eight layouts are ready.',
    )
    if (next) {
      snapshot = next
      status = { ...status, bootstrap_ready: true }
    }
  }

  async function reconnect() {
    await perform('reconnect', async () => {
      await api.reconnect()
      notify('info', 'Reconnecting to OBS…')
    })
  }

  async function saveObs(host: string, port: number, password?: string) {
    await perform(
      'settings',
      async () => {
        await api.saveObsSettings(host, port, password && password.length ? password : undefined)
        window.setTimeout(() => void refreshAux(), 650)
      },
      'Connection settings saved. Reconnecting to OBS.',
    )
  }

  async function saveRecording(path: string | null, format: string) {
    await perform(
      'settings',
      async () => {
        await api.saveRecordingSettings(path, format)
        window.setTimeout(() => void refreshAux(), 200)
      },
      'Recording settings saved.',
    )
  }

  async function saveStream(server: string | null, key?: string) {
    await perform(
      'settings',
      async () => {
        await api.saveStreamSettings(server, key)
        window.setTimeout(() => void refreshAux(), 200)
      },
      key === '' ? 'Stream key removed.' : 'Stream destination saved.',
    )
  }

  async function savePreset(name: string) {
    const next = await perform(
      'preset',
      () => api.savePreset(name),
      `Preset “${name}” saved.`,
    )
    if (next) presets = next
  }

  async function loadPreset(name: string) {
    const next = await perform('preset', () => api.loadPreset(name))
    if (next) snapshot = next
  }
</script>

<svelte:head>
  <title>Vidyst Studio</title>
</svelte:head>


<div class="app">
  <TopBar
    {status}
    {snapshot}
    {aspects}
    {activeLayoutName}
    {busy}
    onToggleAspect={toggleAspect}
    onRecord={toggleRecording}
    onLive={toggleStreaming}
    onSplit={splitRecording}
    onProjector={openProjector}
    onSettings={() => (settingsOpen = true)}
    onAudio={() => scrollRail('audio-mixer')}
    onEdit={() => scrollRail('quick-customize')}
  />

  {#if !status.connected}
    <div class="banner offline">
      <span class="banner-icon"><Icon name={status.state === 'connecting' ? 'refresh' : 'alert'} size={15} /></span>
      <div class="banner-copy">
        <strong>{status.state === 'connecting' ? 'Connecting to OBS…' : 'OBS is offline'}</strong>
        <span>{status.message}</span>
      </div>
      <button type="button" class="banner-btn" onclick={reconnect} disabled={busy === 'reconnect'}>
        Retry
      </button>
      <button type="button" class="banner-btn ghost" onclick={() => (settingsOpen = true)}>
        Connection settings
      </button>
    </div>
  {:else if !status.bootstrap_ready}
    <div class="banner setup">
      <span class="banner-icon"><Icon name="sparkles" size={15} /></span>
      <div class="banner-copy">
        <strong>Scene setup needs attention</strong>
        <span>{status.message}</span>
      </div>
      <button type="button" class="banner-btn" onclick={prepareScenes} disabled={busy === 'prepare'}>
        {busy === 'prepare' ? 'Preparing…' : 'Prepare scenes'}
      </button>
    </div>
  {/if}

  <div class="workspace">
    <main class="body">
      <div class="stage-column">
        <PreviewStage
          layout={previewLayout}
          {snapshot}
          {aspects}
          live={previewLive}
          connected={status.connected && isTauri}
          horizontalFrame={horizontalFrame}
          verticalFrame={verticalFrame}
          verticalError={verticalPreviewError}
          onProjector={openProjector}
        />
      </div>
      <RightRail
        {snapshot}
        {activeLayout}
        {devices}
        {devicesLoading}
        {presets}
        {audioMeters}
        {busy}
        disabled={controlsDisabled}
        onMute={mute}
        onMonitor={monitor}
        onVolume={volume}
        onQuickResize={quickResize}
        onBackground={setBackground}
        onBackgroundLayer={setBackgroundLayer}
        onBackgroundImage={chooseBackgroundImage}
        onCamera={setCameraSource}
        onCameraConnected={setCameraConnected}
        onScreen={setScreenSource}
        onScreenConnected={setScreenConnected}
        onVisibility={setVisibility}
        onRefreshDevices={loadDevices}
        onSavePreset={savePreset}
        onLoadPreset={loadPreset}
      />
    </main>

    <RecordingBar
      {snapshot}
      busy={busy}
      disabled={controlsDisabled}
      onChangeFolder={changeRecordFolder}
      onOpenFolder={openRecordFolder}
    />

    <LayoutDock
      {layouts}
      activeLayout={snapshot?.active_layout ?? null}
      {aspects}
      onSelect={selectLayout}
      disabled={controlsDisabled}
      busy={busy === 'layout'}
    />
  </div>

  <SettingsModal
    open={settingsOpen}
    {settings}
    {status}
    {busy}
    onClose={() => (settingsOpen = false)}
    onSaveObs={saveObs}
    onSaveRecording={saveRecording}
    onSaveStream={saveStream}
    onReconnect={reconnect}
    onPrepareScenes={prepareScenes}
  />

  <Toasts {toasts} onDismiss={dismissToast} />

  {#if initializing}
    <div class="boot" aria-live="polite">
      <span class="boot-spinner"></span>
      <span>Starting Vidyst Studio…</span>
    </div>
  {/if}
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .workspace {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 12px 12px;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 10px;
  }

  .stage-column {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: block;
  }

  .banner {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 8px 12px 0;
    padding: 8px 11px;
    border-radius: 9px;
    border: 1px solid var(--border-soft);
    background: var(--surface-1);
    box-shadow: var(--shadow-panel);
  }

  .banner.offline {
    border-color: rgba(255, 105, 105, 0.19);
    background: color-mix(in srgb, #2a1418 72%, var(--surface-1));
  }

  .banner.setup {
    border-color: rgba(242, 184, 93, 0.18);
    background: color-mix(in srgb, #2b2314 62%, var(--surface-1));
  }

  .banner-icon {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    flex: none;
    color: #ff9696;
    background: rgba(255, 94, 94, 0.09);
  }

  .banner.setup .banner-icon {
    color: #ecc078;
    background: rgba(242, 184, 93, 0.08);
  }

  .banner.offline .banner-icon :global(svg) {
    animation: spin 1.4s linear infinite;
  }

  .banner-copy {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .banner-copy strong {
    font-size: 9.5px;
    font-weight: 650;
    color: var(--text-muted);
  }

  .banner-copy span {
    font-size: 8.5px;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .banner-btn {
    flex: none;
    height: 26px;
    padding: 0 9px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 9px;
    font-weight: 550;
    cursor: pointer;
  }

  .banner-btn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--border-strong);
    background: var(--surface-3);
  }

  .banner-btn.ghost {
    background: transparent;
    border-color: transparent;
    color: var(--text-faint);
  }

  .banner-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .boot {
    position: fixed;
    inset: 0;
    z-index: 150;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    background: var(--surface-0);
    color: var(--text-faint);
    font-size: 10px;
  }

  .boot-spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid var(--border);
    border-top-color: var(--accent);
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 920px) {
    .banner-btn.ghost {
      display: none;
    }
  }
</style>
