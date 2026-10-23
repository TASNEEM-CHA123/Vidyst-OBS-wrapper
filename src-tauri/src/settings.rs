use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const KEYRING_SERVICE: &str = "com.vidyst.obs.stream-key";
const KEYRING_USER: &str = "default";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedSettings {
    #[serde(default = "default_host")]
    pub obs_host: String,
    #[serde(default = "default_port")]
    pub obs_port: u16,
    #[serde(default = "default_record_format")]
    pub record_format: String,
    #[serde(default)]
    pub record_path: Option<String>,
    #[serde(default)]
    pub stream_server: Option<String>,
    #[serde(default = "default_screen_mode")]
    pub screen_mode: String,
    #[serde(default)]
    pub window_value: Option<String>,
    #[serde(default)]
    pub window_name: Option<String>,
    #[serde(default = "default_true")]
    pub camera_connected: bool,
    #[serde(default = "default_true")]
    pub screen_connected: bool,
    #[serde(default)]
    pub camera_device_value: Option<String>,
    #[serde(default)]
    pub camera_device_name: Option<String>,
    #[serde(default)]
    pub screen_device_value: Option<String>,
    #[serde(default)]
    pub screen_device_name: Option<String>,
    #[serde(default = "default_background_mode")]
    pub background_mode: String,
    #[serde(default = "default_background_color")]
    pub background_color: String,
    #[serde(default)]
    pub background_image: Option<String>,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            obs_host: default_host(),
            obs_port: default_port(),
            record_format: default_record_format(),
            record_path: None,
            stream_server: None,
            screen_mode: default_screen_mode(),
            window_value: None,
            window_name: None,
            camera_connected: true,
            screen_connected: true,
            camera_device_value: None,
            camera_device_name: None,
            screen_device_value: None,
            screen_device_name: None,
            background_mode: default_background_mode(),
            background_color: default_background_color(),
            background_image: None,
        }
    }
}

fn default_screen_mode() -> String {
    "display".to_owned()
}

fn default_true() -> bool {
    true
}

fn default_background_mode() -> String {
    "none".to_owned()
}

fn default_background_color() -> String {
    "#0b0d12".to_owned()
}

fn default_host() -> String {
    "127.0.0.1".to_owned()
}

fn default_port() -> u16 {
    4455
}

fn default_record_format() -> String {
    "hybrid_mp4".to_owned()
}

pub fn config_dir(app: &AppHandle) -> Result<PathBuf> {
    // Local development can point the app at an isolated state folder so test
    // runs never overwrite the user's real settings.
    if let Some(dir) = std::env::var_os("VIDYST_CONFIG_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    app.path()
        .app_config_dir()
        .context("resolve the application config directory")
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(config_dir(app)?.join("settings.json"))
}

pub fn load(app: &AppHandle) -> PersistedSettings {
    let Ok(path) = settings_path(app) else {
        return PersistedSettings::default();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return PersistedSettings::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &PersistedSettings) -> Result<()> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create settings directory {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(settings)?;
    write_atomic(&path, text.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, bytes).with_context(|| format!("write {}", temp.display()))?;
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("replace {}", path.display()))?;
    }
    fs::rename(&temp, path).with_context(|| format!("rename {}", temp.display()))?;
    Ok(())
}

/// Reads the stream key from the platform credential store. Any failure is
/// treated as "no key configured"; the secret is never logged or serialized.
pub fn read_stream_key() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
    entry
        .get_password()
        .ok()
        .filter(|key| !key.trim().is_empty())
}

pub fn write_stream_key(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("open the platform credential store")?;
    entry
        .set_password(key)
        .context("store the stream key in the platform credential store")
}

pub fn clear_stream_key() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("open the platform credential store")?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        // Deleting an absent credential is a successful no-op.
        Err(_) => Ok(()),
    }
}

#[derive(Debug, Serialize)]
pub struct SettingsView {
    pub obs_host: String,
    pub obs_port: u16,
    pub has_obs_password: bool,
    pub record_format: String,
    pub record_path: Option<String>,
    pub stream_server: Option<String>,
    pub has_stream_key: bool,
    pub settings_path: String,
}

pub fn view(app: &AppHandle, password: Option<&str>, settings: &PersistedSettings) -> SettingsView {
    SettingsView {
        obs_host: settings.obs_host.clone(),
        obs_port: settings.obs_port,
        has_obs_password: password.is_some_and(|value| !value.is_empty()),
        record_format: settings.record_format.clone(),
        record_path: settings.record_path.clone(),
        stream_server: settings.stream_server.clone(),
        has_stream_key: read_stream_key().is_some(),
        settings_path: settings_path(app)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    }
}
