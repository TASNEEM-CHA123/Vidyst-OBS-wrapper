export type AspectRatio = 'R16x9' | 'R9x16' | 'R1x1' | 'R4x5'
export type LayoutGroup = 'Solo' | 'Screen'
export type SlotSource = 'Screen' | 'Camera' | 'Background'
export type Fit = 'Cover' | 'Contain'

export interface Rect {
  x: number
  y: number
  w: number
  h: number
}

export interface Slot {
  source: SlotSource
  rect: Rect
  radius_pct: number
  fit: Fit
  z: number
}

export interface Layout {
  id: string
  name: string
  group: LayoutGroup
  slots: Record<string, Slot[]>
}

export interface ConnectionStatus {
  state: 'connecting' | 'connected' | 'disconnected' | 'error' | string
  message: string
  connected: boolean
  obs_version: string | null
  websocket_version: string | null
  rpc_version: number | null
  reconnect_attempt: number
  bootstrap_ready: boolean
}

export interface AudioChannel {
  input: string
  label: string
  muted: boolean
  monitoring: boolean
  monitor_type: string
  volume_db: number
  volume_mul: number
}

export interface DeviceSelection {
  name: string
  value: string
}

export interface StudioSnapshot {
  connected: boolean
  bootstrap_ready: boolean
  obs_version: string | null
  websocket_version: string | null
  program_scene: string | null
  vertical_scene: string | null
  active_layout: string | null
  active_layout_name: string | null
  active_vertical_layout: string | null
  transition: string | null
  transition_duration_ms: number
  recording: boolean
  recording_horizontal: boolean
  recording_vertical: boolean
  recording_paused: boolean
  recording_timecode: string
  recording_directory: string | null
  recording_format: string
  last_recording_path: string | null
  streaming: boolean
  streaming_horizontal: boolean
  streaming_vertical: boolean
  stream_reconnecting: boolean
  stream_timecode: string
  camera: DeviceSelection | null
  camera_connected: boolean
  screen: DeviceSelection | null
  screen_connected: boolean
  screen_mode: 'display' | 'window' | string
  window: DeviceSelection | null
  camera_visible: boolean
  screen_visible: boolean
  camera_radius: number
  background_removal: boolean
  background_mode: 'none' | 'color' | 'image' | string
  background_color: string
  background_image: string | null
  audio: AudioChannel[]
  stream_key_configured: boolean
}

export interface DeviceOption {
  name: string
  value: string
}

export interface DeviceLists {
  cameras: DeviceOption[]
  screens: DeviceOption[]
  windows: DeviceOption[]
}

export interface QuickResizeRequest {
  corner?: string | null
  size_pct?: number | null
  shape?: string | null
  split_pct?: number | null
}

export interface SettingsView {
  obs_host: string
  obs_port: number
  has_obs_password: boolean
  record_format: string
  record_path: string | null
  stream_server: string | null
  has_stream_key: boolean
  settings_path: string
}

export interface PluginStatus {
  key: string
  name: string
  required: boolean
  installed: boolean
  evidence: string[]
  install_url: string
  note: string | null
}

export interface DiscoveryReport {
  websocket_reachable: boolean
  ready: boolean
  plugins: PluginStatus[]
  missing_plugins: string[]
  warnings: string[]
  runtime: {
    obs_studio_version: string
    obs_websocket_version: string
    rpc_version: number
    platform_description: string
    input_kind_count: number
    filter_kind_count: number
    transition_kind_count: number
    canvas_list: Array<{ name: string; width: number; height: number }>
  }
}

export interface LayoutPreset {
  name: string
  layout_id: string
  camera: {
    x: number
    y: number
    w: number
    h: number
    radius_pct: number
  } | null
  split_pct: number | null
  created_at_unix: number
}

export interface StudioEvent {
  kind: string
  detail: unknown
  at_ms: number
}

export interface PreviewFrame {
  target: 'horizontal' | 'vertical'
  image: string
  width: number
  height: number
  at_ms: number
}

export interface AudioMeter {
  input: string
  peak_db: number
}

export interface AudioMetersPayload {
  meters: AudioMeter[]
  at_ms: number
}

export type AspectTargets = {
  horizontal: boolean
  vertical: boolean
}

export interface Toast {
  id: number
  kind: 'info' | 'success' | 'error'
  message: string
}
