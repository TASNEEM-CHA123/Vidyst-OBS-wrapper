import type {
  AudioChannel,
  ConnectionStatus,
  DeviceLists,
  DiscoveryReport,
  Layout,
  LayoutPreset,
  SettingsView,
  Slot,
  StudioSnapshot,
} from './types'

function slot(
  source: 'Screen' | 'Camera',
  x: number,
  y: number,
  w: number,
  h: number,
  radius: number,
  fit: 'Cover' | 'Contain' = source === 'Screen' ? 'Contain' : 'Cover',
): Slot {
  return {
    source,
    rect: { x, y, w, h },
    radius_pct: radius,
    fit,
    z: source === 'Camera' ? 1 : 0,
  }
}

function layout(
  id: string,
  name: string,
  group: 'Solo' | 'Screen',
  horizontal: Slot[],
  vertical: Slot[],
): Layout {
  return { id, name, group, slots: { R16x9: horizontal, R9x16: vertical } }
}

export const mockLayouts: Layout[] = [
  layout(
    'just_me',
    'Just me',
    'Solo',
    [slot('Camera', 0.02, 0.03, 0.96, 0.94, 4)],
    [slot('Camera', 0.03, 0.02, 0.94, 0.96, 4)],
  ),
  layout(
    'screen_cam',
    'Screen + cam',
    'Screen',
    [slot('Screen', 0.02, 0.03, 0.96, 0.94, 3), slot('Camera', 0.83, 0.7, 0.140625, 0.25, 50)],
    [slot('Screen', 0.02, 0.08, 0.96, 0.5, 3), slot('Camera', 0.29, 0.6, 0.42, 0.23625, 50)],
  ),
  layout(
    'screen_cam_inv',
    'Screen + cam inverted',
    'Screen',
    [slot('Screen', 0.02, 0.03, 0.96, 0.94, 3), slot('Camera', 0.03, 0.7, 0.140625, 0.25, 50)],
    [slot('Screen', 0.02, 0.4, 0.96, 0.5, 3), slot('Camera', 0.29, 0.06, 0.42, 0.23625, 50)],
  ),
  layout(
    'screen_tile',
    'Screen + tile',
    'Screen',
    [slot('Screen', 0.02, 0.03, 0.96, 0.94, 3), slot('Camera', 0.74, 0.68, 0.22, 0.25, 8)],
    [slot('Screen', 0.03, 0.04, 0.94, 0.52, 3), slot('Camera', 0.03, 0.58, 0.94, 0.38, 6)],
  ),
  layout(
    'screen_tile_inv',
    'Screen + tile inverted',
    'Screen',
    [slot('Screen', 0.02, 0.03, 0.96, 0.94, 3), slot('Camera', 0.04, 0.68, 0.22, 0.25, 8)],
    [slot('Screen', 0.03, 0.44, 0.94, 0.52, 3), slot('Camera', 0.03, 0.04, 0.94, 0.38, 6)],
  ),
  layout(
    'side_by_side',
    'Side by side',
    'Screen',
    [slot('Screen', 0.02, 0.03, 0.71, 0.94, 3), slot('Camera', 0.75, 0.03, 0.23, 0.94, 4)],
    [slot('Screen', 0.03, 0.04, 0.94, 0.52, 3), slot('Camera', 0.03, 0.58, 0.94, 0.38, 4)],
  ),
  layout(
    'side_by_side_inv',
    'Side by side inverted',
    'Screen',
    [slot('Screen', 0.27, 0.03, 0.71, 0.94, 3), slot('Camera', 0.02, 0.03, 0.23, 0.94, 4)],
    [slot('Screen', 0.03, 0.44, 0.94, 0.52, 3), slot('Camera', 0.03, 0.04, 0.94, 0.38, 4)],
  ),
  layout(
    'screen_focus',
    'Screen focus',
    'Screen',
    [slot('Screen', 0.02, 0.03, 0.96, 0.94, 3)],
    [slot('Screen', 0.02, 0.2, 0.96, 0.6, 3)],
  ),
]

const mockAudio: AudioChannel[] = [
  {
    input: 'Vidyst_Mic',
    label: 'External mic',
    muted: false,
    monitoring: false,
    monitor_type: 'OBS_MONITORING_TYPE_NONE',
    volume_db: -6,
    volume_mul: 0.5,
  },
  {
    input: 'Vidyst_Screen_Audio',
    label: 'Screen audio',
    muted: false,
    monitoring: false,
    monitor_type: 'OBS_MONITORING_TYPE_NONE',
    volume_db: -14,
    volume_mul: 0.2,
  },
]

export const mockStatus: ConnectionStatus = {
  state: 'connected',
  message: 'Demo mode — OBS connection is simulated.',
  connected: true,
  obs_version: '32.2.2',
  websocket_version: '5.7.4',
  rpc_version: 1,
  reconnect_attempt: 0,
  bootstrap_ready: true,
}

let activeLayout = 'screen_cam'
let recording = false
let recordingHorizontal = false
let recordingVertical = false
let streaming = false
let streamingHorizontal = false
let streamingVertical = false
let cameraConnected = true
let screenConnected = true
let backgroundRemoval = false
let audio = structuredClone(mockAudio)
let cameraShape = 'circle'
let cameraSize = 25
let cameraCorner = 'br'
let split = 71
let screenMode = 'display'
let windowSelection: { name: string; value: string } | null = null
let backgroundMode = 'none'
let backgroundColor = '#0b0d12'
let backgroundImage: string | null = null

export const mockSnapshot = (): StudioSnapshot => {
  const layout = mockLayouts.find((item) => item.id === activeLayout) ?? mockLayouts[0]
  return {
    connected: true,
    bootstrap_ready: true,
    obs_version: '32.2.2',
    websocket_version: '5.7.4',
    program_scene: `Vidyst_${layout.id}`,
    vertical_scene: `Vidyst_${layout.id}`,
    active_layout: layout.id,
    active_layout_name: layout.name,
    active_vertical_layout: layout.id,
    transition: 'Move',
    transition_duration_ms: 350,
    recording,
    recording_horizontal: recordingHorizontal,
    recording_vertical: recordingVertical,
    recording_paused: false,
    recording_timecode: '00:00:00',
    recording_directory: 'C:\\Users\\You\\Videos',
    recording_format: 'hybrid_mp4',
    last_recording_path: recording ? null : 'C:\\Users\\You\\Videos\\Vidyst 2026-10-22 10-13-26.mp4',
    streaming,
    streaming_horizontal: streamingHorizontal,
    streaming_vertical: streamingVertical,
    stream_reconnecting: false,
    stream_timecode: '00:00:00',
    stream_state: {
      phase: streaming ? 'Live' : 'Idle',
      requested_horizontal: streamingHorizontal,
      requested_vertical: streamingVertical,
      horizontal_live: streamingHorizontal,
      vertical_live: streamingVertical,
      failed_targets: [],
      message: null,
      retry_available: false,
      retry_mode: null,
      updated_at_ms: 0,
    },
    camera: { name: 'HP Wide Vision HD Camera', value: 'demo-camera' },
    camera_connected: cameraConnected,
    screen: { name: '1920x1080 @ 0,0 (Primary Monitor)', value: 'demo-screen' },
    screen_connected: screenConnected,
    screen_mode: screenMode,
    window: windowSelection,
    camera_visible: layout.id !== 'screen_focus',
    screen_visible: layout.id !== 'just_me',
    camera_radius: cameraShape === 'circle' ? 540 : 86.4,
    background_removal: backgroundRemoval,
    background_removal_available: true,
    background_mode: backgroundMode,
    background_color: backgroundColor,
    background_image: backgroundImage,
    audio,
    stream_key_configured: true,
  }
}

export const mockSettings = (): SettingsView => ({
  obs_host: '127.0.0.1',
  obs_port: 4455,
  has_obs_password: true,
  record_format: 'hybrid_mp4',
  record_path: 'C:\\Users\\You\\Videos',
  stream_server: 'rtmp://live.example.com/app',
  has_stream_key: true,
  preview_fps: 15,
  settings_path: 'C:\\Users\\You\\AppData\\Roaming\\com.vidyst.obs\\settings.json',
})

export const mockDevices = (): DeviceLists => ({
  cameras: [
    { name: 'HP Wide Vision HD Camera', value: 'demo-camera' },
    { name: 'Virtual Camera', value: 'demo-virtual' },
  ],
  screens: [
    { name: '1920x1080 @ 0,0 (Primary Monitor)', value: 'demo-screen' },
    { name: '2560x1440 @ 1920,0 (Secondary Monitor)', value: 'demo-screen-2' },
  ],
  windows: [
    { name: 'Figma — Vidyst Studio', value: 'demo-window-figma' },
    { name: 'Google Chrome — Slides', value: 'demo-window-chrome' },
  ],
})

export const mockDiscovery = (): DiscoveryReport => ({
  websocket_reachable: true,
  ready: true,
  plugins: [
    ['move_transition', 'Move Transition'],
    ['advanced_masks', 'Advanced Masks'],
    ['aitum_vertical', 'Aitum Vertical'],
    ['background_removal', 'Background Removal'],
    ['multi_rtmp', 'Multi RTMP'],
  ].map(([key, name]) => ({
    key,
    name,
    required: true,
    installed: true,
    evidence: ['file: demo/plugin.dll'],
    install_url: '',
    note: null,
  })),
  missing_plugins: [],
  warnings: [],
  runtime: {
    obs_studio_version: '32.2.2',
    obs_websocket_version: '5.7.4',
    rpc_version: 1,
    platform_description: 'Windows 11',
    input_kind_count: 62,
    filter_kind_count: 45,
    transition_kind_count: 18,
    canvas_list: [
      { name: 'Main', width: 1920, height: 1080 },
      { name: 'Aitum Vertical', width: 1080, height: 1920 },
    ],
  },
})

let presets: LayoutPreset[] = [
  {
    name: 'Podcast wide',
    layout_id: 'side_by_side',
    camera: { x: 0.75, y: 0.03, w: 0.23, h: 0.94, radius_pct: 86.4 },
    split_pct: 71,
    created_at_unix: Math.floor(Date.now() / 1000),
  },
]

export async function mockInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((resolve) => setTimeout(resolve, 65))
  switch (command) {
    case 'get_status':
      return mockStatus as T
    case 'get_studio_state':
      return mockSnapshot() as T
    case 'get_layouts':
      return mockLayouts as T
    case 'get_settings':
      return mockSettings() as T
    case 'save_obs_settings':
    case 'save_recording_settings':
    case 'save_stream_settings':
    case 'reconnect':
    case 'open_projector':
    case 'split_recording':
      return {
        horizontal_path: 'native split',
        vertical_path: 'inactive',
        gapless: true,
        message: 'Recording split seamlessly.',
      } as T
    case 'prepare_scenes':
      return mockSnapshot() as T
    case 'apply_layout': {
      activeLayout = String(args?.id ?? activeLayout)
      return mockSnapshot() as T
    }
    case 'quick_resize_camera': {
      const request = args?.request as Record<string, unknown> | undefined
      if (request?.shape) cameraShape = String(request.shape)
      if (request?.size_pct) cameraSize = Number(request.size_pct)
      if (request?.corner) cameraCorner = String(request.corner)
      if (request?.split_pct) split = Number(request.split_pct)
      return mockSnapshot() as T
    }
    case 'set_audio_mute':
      audio = audio.map((channel) =>
        channel.input === args?.input ? { ...channel, muted: Boolean(args?.muted) } : channel,
      )
      return undefined as T
    case 'set_audio_monitor':
      audio = audio.map((channel) =>
        channel.input === args?.input
          ? {
              ...channel,
              monitoring: Boolean(args?.monitoring),
              monitor_type: args?.monitoring
                ? 'OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT'
                : 'OBS_MONITORING_TYPE_NONE',
            }
          : channel,
      )
      return undefined as T
    case 'set_audio_volume':
      audio = audio.map((channel) =>
        channel.input === args?.input ? { ...channel, volume_db: Number(args?.db ?? 0) } : channel,
      )
      return undefined as T
    case 'get_audio':
      return audio as T
    case 'toggle_background_removal':
      backgroundRemoval = Boolean(args?.enabled)
      return mockSnapshot() as T
    case 'list_devices':
      return mockDevices() as T
    case 'set_camera_source':
      cameraConnected = true
      return mockSnapshot() as T
    case 'set_camera_connected':
      cameraConnected = Boolean(args?.connected)
      return mockSnapshot() as T
    case 'set_screen_source':
      screenMode = 'display'
      screenConnected = true
      windowSelection = null
      return mockSnapshot() as T
    case 'set_screen_connected':
      screenConnected = Boolean(args?.connected)
      return mockSnapshot() as T
    case 'set_window_source':
      screenMode = 'window'
      screenConnected = true
      windowSelection = {
        value: String(args?.value ?? ''),
        name: String(args?.name ?? 'Application window'),
      }
      return mockSnapshot() as T
    case 'set_background':
      backgroundMode = String(args?.mode ?? 'none')
      backgroundColor = String(args?.color ?? backgroundColor)
      if (args?.image) backgroundImage = String(args.image)
      return mockSnapshot() as T
    case 'pick_background_image':
      backgroundImage = 'C:\\Users\\You\\Pictures\\studio-background.png'
      backgroundMode = 'image'
      return mockSnapshot() as T
    case 'set_preview_active':
      return undefined as T
    case 'pick_record_folder':
      return mockSnapshot() as T
    case 'open_record_folder':
      return undefined as T
    case 'set_source_visibility':
      return mockSnapshot() as T
    case 'start_recording':
      recording = true
      recordingHorizontal = Boolean(args?.horizontal)
      recordingVertical = Boolean(args?.vertical)
      return undefined as T
    case 'stop_recording':
      recording = false
      recordingHorizontal = false
      recordingVertical = false
      return undefined as T
    case 'start_streaming':
      streaming = true
      streamingHorizontal = Boolean(args?.horizontal)
      streamingVertical = Boolean(args?.vertical)
      return undefined as T
    case 'stop_streaming':
      streaming = false
      streamingHorizontal = false
      streamingVertical = false
      return undefined as T
    case 'get_plugin_health':
      return mockDiscovery() as T
    case 'list_presets':
      return presets as T
    case 'save_preset': {
      const name = String(args?.name ?? 'Untitled')
      presets = [
        ...presets.filter((preset) => preset.name !== name),
        {
          name,
          layout_id: activeLayout,
          camera: { x: 0.03, y: 0.03, w: 0.2, h: 0.4, radius_pct: cameraShape === 'circle' ? 540 : 86.4 },
          split_pct: activeLayout.startsWith('side_by_side') ? split : null,
          created_at_unix: Math.floor(Date.now() / 1000),
        },
      ]
      return presets as T
    }
    case 'load_preset': {
      const preset = presets.find((item) => item.name === args?.name)
      if (preset) activeLayout = preset.layout_id
      return mockSnapshot() as T
    }
    default:
      return undefined as T
  }
}

export const mockMeta = {
  get cameraSize() {
    return cameraSize
  },
  get cameraCorner() {
    return cameraCorner
  },
  get split() {
    return split
  },
}
