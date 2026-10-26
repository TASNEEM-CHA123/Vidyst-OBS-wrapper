mod commands;
mod obs;
mod settings;

use std::sync::Arc;

use tauri::Manager;

use crate::obs::ObsService;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let service = ObsService::new(app.handle().clone());
            app.manage(Arc::clone(&service));
            service.start();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_studio_state,
            commands::get_layouts,
            commands::get_settings,
            commands::save_obs_settings,
            commands::reconnect,
            commands::prepare_scenes,
            commands::apply_layout,
            commands::quick_resize_camera,
            commands::set_audio_mute,
            commands::set_audio_monitor,
            commands::set_audio_volume,
            commands::get_audio,
            commands::toggle_background_removal,
            commands::list_devices,
            commands::set_camera_source,
            commands::set_camera_connected,
            commands::set_screen_source,
            commands::set_screen_connected,
            commands::set_window_source,
            commands::set_background,
            commands::pick_background_image,
            commands::set_preview_active,
            commands::save_preview_settings,
            commands::pick_record_folder,
            commands::open_record_folder,
            commands::set_source_visibility,
            commands::start_recording,
            commands::stop_recording,
            commands::split_recording,
            commands::start_streaming,
            commands::stop_streaming,
            commands::retry_streaming,
            commands::open_projector,
            commands::get_plugin_health,
            commands::save_recording_settings,
            commands::save_stream_settings,
            commands::list_presets,
            commands::save_preset,
            commands::load_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vidyst Studio");
}
