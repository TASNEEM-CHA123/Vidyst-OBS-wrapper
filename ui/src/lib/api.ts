import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { mockInvoke } from './mock'
import type {
  ConnectionStatus,
  DeviceLists,
  DiscoveryReport,
  Layout,
  LayoutPreset,
  QuickResizeRequest,
  SettingsView,
  StudioSnapshot,
} from './types'

export const isTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri) return mockInvoke<T>(name, args)
  return invoke<T>(name, args)
}

export async function onStudioEvent<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!isTauri) return () => {}
  return listen<T>(event, (message) => handler(message.payload))
}

export const api = {
  status: () => command<ConnectionStatus>('get_status'),
  studio: () => command<StudioSnapshot>('get_studio_state'),
  layouts: () => command<Layout[]>('get_layouts'),
  settings: () => command<SettingsView>('get_settings'),
  saveObsSettings: (host: string, port: number, password?: string) =>
    command<void>('save_obs_settings', {
      host,
      port,
      password: password === undefined ? null : password,
    }),
  reconnect: () => command<void>('reconnect'),
  prepareScenes: () => command<StudioSnapshot>('prepare_scenes'),
  applyLayout: (id: string, horizontal: boolean, vertical: boolean) =>
    command<StudioSnapshot>('apply_layout', { id, horizontal, vertical }),
  quickResize: (request: QuickResizeRequest) =>
    command<StudioSnapshot>('quick_resize_camera', { request }),
  setMute: (input: string, muted: boolean) =>
    command<void>('set_audio_mute', { input, muted }),
  setMonitor: (input: string, monitoring: boolean) =>
    command<void>('set_audio_monitor', { input, monitoring }),
  setVolume: (input: string, db: number) =>
    command<void>('set_audio_volume', { input, db }),
  toggleBackgroundRemoval: (enabled: boolean) =>
    command<StudioSnapshot>('toggle_background_removal', { enabled }),
  devices: () => command<DeviceLists>('list_devices'),
  setCameraSource: (value: string) =>
    command<StudioSnapshot>('set_camera_source', { value }),
  setCameraConnected: (connected: boolean) =>
    command<StudioSnapshot>('set_camera_connected', { connected }),
  setScreenSource: (value: string) =>
    command<StudioSnapshot>('set_screen_source', { value }),
  setScreenConnected: (connected: boolean) =>
    command<StudioSnapshot>('set_screen_connected', { connected }),
  setWindowSource: (value: string, name: string) =>
    command<StudioSnapshot>('set_window_source', { value, name }),
  setBackground: (mode: 'none' | 'color' | 'image', color: string, image: string | null = null) =>
    command<StudioSnapshot>('set_background', { mode, color, image }),
  pickBackgroundImage: () => command<StudioSnapshot | null>('pick_background_image'),
  setPreviewActive: (active: boolean, horizontal: boolean, vertical: boolean) =>
    command<void>('set_preview_active', { active, horizontal, vertical }),
  setVisibility: (source: string, visible: boolean) =>
    command<StudioSnapshot>('set_source_visibility', { source, visible }),
  startRecording: (horizontal: boolean, vertical: boolean) =>
    command<void>('start_recording', { horizontal, vertical }),
  stopRecording: () => command<void>('stop_recording'),
  splitRecording: () => command<void>('split_recording'),
  startStreaming: (horizontal: boolean, vertical: boolean) =>
    command<void>('start_streaming', { horizontal, vertical }),
  stopStreaming: () => command<void>('stop_streaming'),
  projector: (target: 'horizontal' | 'vertical') =>
    command<void>('open_projector', { target }),
  pluginHealth: () => command<DiscoveryReport>('get_plugin_health'),
  saveRecordingSettings: (path: string | null, format: string) =>
    command<void>('save_recording_settings', { path, format }),
  pickRecordFolder: () => command<StudioSnapshot | null>('pick_record_folder'),
  openRecordFolder: () => command<void>('open_record_folder'),
  saveStreamSettings: (server: string | null, key?: string) =>
    command<void>('save_stream_settings', {
      server,
      key: key === undefined ? null : key,
    }),
  presets: () => command<LayoutPreset[]>('list_presets'),
  savePreset: (name: string) => command<LayoutPreset[]>('save_preset', { name }),
  loadPreset: (name: string) => command<StudioSnapshot>('load_preset', { name }),
}
