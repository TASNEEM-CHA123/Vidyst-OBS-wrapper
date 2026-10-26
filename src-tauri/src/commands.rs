use std::{path::Path, process::Command, sync::Arc};

use tauri::State;
use vidyst_layouts::Layout;
use vidyst_obs_driver::discovery::DiscoveryReport;

use crate::{
    obs::{
        AudioChannel, ConnectionStatus, DeviceLists, LayoutPreset, ObsService, QuickResizeRequest,
        SplitRecordingReport, StudioSnapshot,
    },
    settings::SettingsView,
};

fn error(error: impl std::fmt::Display) -> String {
    format!("{error:#}")
}

#[tauri::command]
pub async fn get_status(service: State<'_, Arc<ObsService>>) -> Result<ConnectionStatus, String> {
    Ok(service.status().await)
}

#[tauri::command]
pub async fn get_studio_state(
    service: State<'_, Arc<ObsService>>,
) -> Result<StudioSnapshot, String> {
    service.snapshot().await.map_err(error)
}

#[tauri::command]
pub async fn get_layouts(service: State<'_, Arc<ObsService>>) -> Result<Vec<Layout>, String> {
    Ok(service.layouts().await)
}

#[tauri::command]
pub async fn get_settings(service: State<'_, Arc<ObsService>>) -> Result<SettingsView, String> {
    Ok(service.settings_view().await)
}

#[tauri::command]
pub async fn save_obs_settings(
    service: State<'_, Arc<ObsService>>,
    host: String,
    port: u16,
    password: Option<String>,
) -> Result<(), String> {
    service
        .update_obs_config(host, port, password)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn reconnect(service: State<'_, Arc<ObsService>>) -> Result<(), String> {
    service.reconnect().await;
    Ok(())
}

#[tauri::command]
pub async fn prepare_scenes(service: State<'_, Arc<ObsService>>) -> Result<StudioSnapshot, String> {
    service.prepare_scenes().await.map_err(error)
}

#[tauri::command]
pub async fn apply_layout(
    service: State<'_, Arc<ObsService>>,
    id: String,
    horizontal: bool,
    vertical: bool,
) -> Result<StudioSnapshot, String> {
    service
        .apply_layout(id, horizontal, vertical)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn quick_resize_camera(
    service: State<'_, Arc<ObsService>>,
    request: QuickResizeRequest,
) -> Result<StudioSnapshot, String> {
    service.quick_resize(request).await.map_err(error)
}

#[tauri::command]
pub async fn set_audio_mute(
    service: State<'_, Arc<ObsService>>,
    input: String,
    muted: bool,
) -> Result<(), String> {
    service.set_audio_mute(input, muted).await.map_err(error)
}

#[tauri::command]
pub async fn set_audio_monitor(
    service: State<'_, Arc<ObsService>>,
    input: String,
    monitoring: bool,
) -> Result<(), String> {
    service
        .set_audio_monitor(input, monitoring)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn set_audio_volume(
    service: State<'_, Arc<ObsService>>,
    input: String,
    db: f32,
) -> Result<(), String> {
    service.set_audio_volume(input, db).await.map_err(error)
}

#[tauri::command]
pub async fn get_audio(service: State<'_, Arc<ObsService>>) -> Result<Vec<AudioChannel>, String> {
    Ok(service.snapshot().await.map_err(error)?.audio)
}

#[tauri::command]
pub async fn toggle_background_removal(
    service: State<'_, Arc<ObsService>>,
    enabled: bool,
) -> Result<StudioSnapshot, String> {
    service
        .toggle_background_removal(enabled)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn list_devices(service: State<'_, Arc<ObsService>>) -> Result<DeviceLists, String> {
    service.list_devices().await.map_err(error)
}

#[tauri::command]
pub async fn set_camera_source(
    service: State<'_, Arc<ObsService>>,
    value: String,
) -> Result<StudioSnapshot, String> {
    service.set_camera_source(value).await.map_err(error)
}

#[tauri::command]
pub async fn set_camera_connected(
    service: State<'_, Arc<ObsService>>,
    connected: bool,
) -> Result<StudioSnapshot, String> {
    service.set_camera_connected(connected).await.map_err(error)
}

#[tauri::command]
pub async fn set_screen_source(
    service: State<'_, Arc<ObsService>>,
    value: String,
) -> Result<StudioSnapshot, String> {
    service.set_screen_source(value).await.map_err(error)
}

#[tauri::command]
pub async fn set_screen_connected(
    service: State<'_, Arc<ObsService>>,
    connected: bool,
) -> Result<StudioSnapshot, String> {
    service.set_screen_connected(connected).await.map_err(error)
}

#[tauri::command]
pub async fn set_window_source(
    service: State<'_, Arc<ObsService>>,
    value: String,
    name: String,
) -> Result<StudioSnapshot, String> {
    service.set_window_source(value, name).await.map_err(error)
}

#[tauri::command]
pub async fn set_background(
    service: State<'_, Arc<ObsService>>,
    mode: String,
    color: String,
    image: Option<String>,
) -> Result<StudioSnapshot, String> {
    service
        .set_background(mode, color, image)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn pick_background_image(
    service: State<'_, Arc<ObsService>>,
) -> Result<Option<StudioSnapshot>, String> {
    let snapshot = service.snapshot().await.map_err(error)?;
    let selected = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choose a background image")
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
            .pick_file()
    })
    .await
    .map_err(|error| error.to_string())?;
    let Some(path) = selected else {
        return Ok(None);
    };
    let path = path.display().to_string();
    service
        .set_background("image".to_owned(), snapshot.background_color, Some(path))
        .await
        .map(Some)
        .map_err(error)
}

#[tauri::command]
pub async fn set_preview_active(
    service: State<'_, Arc<ObsService>>,
    active: bool,
    horizontal: bool,
    vertical: bool,
) -> Result<(), String> {
    service.set_preview_active(active, horizontal, vertical);
    Ok(())
}

#[tauri::command]
pub async fn save_preview_settings(
    service: State<'_, Arc<ObsService>>,
    fps: u32,
) -> Result<(), String> {
    service.update_preview_fps(fps).await.map_err(error)
}

#[tauri::command]
pub async fn pick_record_folder(
    service: State<'_, Arc<ObsService>>,
) -> Result<Option<StudioSnapshot>, String> {
    let snapshot = service.snapshot().await.map_err(error)?;
    let selected = tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new().set_title("Choose a recording folder");
        if let Some(directory) = snapshot.recording_directory {
            dialog = dialog.set_directory(directory);
        }
        dialog.pick_folder()
    })
    .await
    .map_err(|error| error.to_string())?;
    let Some(path) = selected else {
        return Ok(None);
    };
    service
        .set_record_directory(path.display().to_string())
        .await
        .map(Some)
        .map_err(error)
}

#[tauri::command]
pub async fn open_record_folder(service: State<'_, Arc<ObsService>>) -> Result<(), String> {
    let snapshot = service.snapshot().await.map_err(error)?;
    if let Some(file) = snapshot
        .last_recording_path
        .as_deref()
        .filter(|path| Path::new(path).is_file())
    {
        Command::new("explorer.exe")
            .arg(format!("/select,{file}"))
            .spawn()
            .map_err(error)?;
        return Ok(());
    }
    let directory = snapshot
        .recording_directory
        .filter(|path| Path::new(path).is_dir())
        .ok_or_else(|| "the recording folder is not available yet".to_owned())?;
    Command::new("explorer.exe")
        .arg(directory)
        .spawn()
        .map_err(error)?;
    Ok(())
}

#[tauri::command]
pub async fn set_source_visibility(
    service: State<'_, Arc<ObsService>>,
    source: String,
    visible: bool,
) -> Result<StudioSnapshot, String> {
    service
        .set_source_visibility(source, visible)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn start_recording(
    service: State<'_, Arc<ObsService>>,
    horizontal: bool,
    vertical: bool,
) -> Result<(), String> {
    service
        .start_recording(horizontal, vertical)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn stop_recording(service: State<'_, Arc<ObsService>>) -> Result<(), String> {
    service.stop_recording().await.map_err(error)
}

#[tauri::command]
pub async fn split_recording(
    service: State<'_, Arc<ObsService>>,
) -> Result<SplitRecordingReport, String> {
    service.split_recording().await.map_err(error)
}

#[tauri::command]
pub async fn start_streaming(
    service: State<'_, Arc<ObsService>>,
    horizontal: bool,
    vertical: bool,
) -> Result<(), String> {
    service
        .start_streaming(horizontal, vertical)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn stop_streaming(service: State<'_, Arc<ObsService>>) -> Result<(), String> {
    service.stop_streaming().await.map_err(error)
}

#[tauri::command]
pub async fn retry_streaming(service: State<'_, Arc<ObsService>>) -> Result<(), String> {
    service.retry_streaming().await.map_err(error)
}

#[tauri::command]
pub async fn open_projector(
    service: State<'_, Arc<ObsService>>,
    target: String,
) -> Result<(), String> {
    service.open_projector(target).await.map_err(error)
}

#[tauri::command]
pub async fn get_plugin_health(
    service: State<'_, Arc<ObsService>>,
) -> Result<DiscoveryReport, String> {
    service.plugin_health().await.map_err(error)
}

#[tauri::command]
pub async fn save_recording_settings(
    service: State<'_, Arc<ObsService>>,
    path: Option<String>,
    format: String,
) -> Result<(), String> {
    service
        .update_recording_config(path, format)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn save_stream_settings(
    service: State<'_, Arc<ObsService>>,
    server: Option<String>,
    key: Option<String>,
) -> Result<(), String> {
    service
        .update_stream_config(server, key)
        .await
        .map_err(error)
}

#[tauri::command]
pub async fn list_presets(
    service: State<'_, Arc<ObsService>>,
) -> Result<Vec<LayoutPreset>, String> {
    Ok(service.list_presets().await)
}

#[tauri::command]
pub async fn save_preset(
    service: State<'_, Arc<ObsService>>,
    name: String,
) -> Result<Vec<LayoutPreset>, String> {
    service.save_preset(name).await.map_err(error)
}

#[tauri::command]
pub async fn load_preset(
    service: State<'_, Arc<ObsService>>,
    name: String,
) -> Result<StudioSnapshot, String> {
    service.load_preset(name).await.map_err(error)
}
