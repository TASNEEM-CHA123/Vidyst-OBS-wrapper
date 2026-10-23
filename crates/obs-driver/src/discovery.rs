//! Environment discovery for the Vidyst-OBS controller.
//!
//! Finds the OBS installation, checks the WebSocket endpoint, verifies the
//! required third-party plugins, and reports OBS/websocket runtime versions.

use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use obws::requests::general::CallVendorRequest;
use obws::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Known locations for an OBS installation.
#[derive(Clone, Debug, Serialize)]
pub struct ObsInstallation {
    pub os: &'static str,
    pub binary: Option<PathBuf>,
    pub system_plugin_dirs: Vec<PathBuf>,
    pub user_plugin_dirs: Vec<PathBuf>,
    pub websocket_config: Option<PathBuf>,
}

impl ObsInstallation {
    pub fn all_plugin_dirs(&self) -> Vec<PathBuf> {
        self.system_plugin_dirs
            .iter()
            .chain(self.user_plugin_dirs.iter())
            .cloned()
            .collect()
    }
}

/// Where the WebSocket password came from (never contains the secret itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum WsCredentials {
    Explicit,
    Environment,
    ObsConfig,
    None,
}

/// Detect OBS paths for the current operating system.
pub fn detect_installation() -> ObsInstallation {
    match std::env::consts::OS {
        "windows" => detect_windows(),
        "macos" => detect_macos(),
        _ => detect_linux(),
    }
}

fn program_files() -> Option<PathBuf> {
    std::env::var_os("ProgramFiles").map(PathBuf::from)
}

fn program_data() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(PathBuf::from)
}

fn appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

fn detect_windows() -> ObsInstallation {
    let mut binaries = Vec::new();
    if let Some(pf) = program_files() {
        binaries.push(pf.join("obs-studio/bin/64bit/obs64.exe"));
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        binaries.push(PathBuf::from(pf86).join("obs-studio/bin/64bit/obs64.exe"));
    }

    let mut system_plugin_dirs = Vec::new();
    if let Some(pd) = program_data() {
        system_plugin_dirs.push(pd.join("obs-studio/plugins"));
    }
    if let Some(pf) = program_files() {
        system_plugin_dirs.push(pf.join("obs-studio/obs-plugins/64bit"));
    }

    let mut user_plugin_dirs = Vec::new();
    let mut websocket_config = None;
    if let Some(ad) = appdata() {
        user_plugin_dirs.push(ad.join("obs-studio/plugins"));
        websocket_config = Some(ad.join("obs-studio/plugin_config/obs-websocket/config.json"));
    }

    ObsInstallation {
        os: "windows",
        binary: first_existing(&binaries),
        system_plugin_dirs,
        user_plugin_dirs,
        websocket_config,
    }
}

fn detect_macos() -> ObsInstallation {
    let home = home_dir().unwrap_or_default();
    let app = PathBuf::from("/Applications/OBS.app");
    let mut system_plugin_dirs = vec![app.join("Contents/PlugIns")];
    system_plugin_dirs.push(PathBuf::from(
        "/Library/Application Support/obs-studio/plugins",
    ));

    ObsInstallation {
        os: "macos",
        binary: first_existing(&[app.join("Contents/MacOS/obs")]),
        system_plugin_dirs,
        user_plugin_dirs: vec![home.join("Library/Application Support/obs-studio/plugins")],
        websocket_config: Some(home.join(
            "Library/Application Support/obs-studio/plugin_config/obs-websocket/config.json",
        )),
    }
}

fn detect_linux() -> ObsInstallation {
    let home = home_dir().unwrap_or_default();
    ObsInstallation {
        os: "linux",
        binary: find_in_path("obs"),
        system_plugin_dirs: vec![
            PathBuf::from("/usr/lib/obs-plugins"),
            PathBuf::from("/usr/lib/x86_64-linux-gnu/obs-plugins"),
            PathBuf::from("/usr/share/obs/obs-plugins"),
        ],
        user_plugin_dirs: vec![
            home.join(".config/obs-studio/plugins"),
            PathBuf::from(
                "/var/lib/flatpak/app/com.obsproject.Studio/current/active/files/lib/obs-plugins",
            ),
        ],
        websocket_config: Some(
            home.join(".config/obs-studio/plugin_config/obs-websocket/config.json"),
        ),
    }
}

fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

/// Read the WebSocket password from an explicit value, `OBS_WS_PASSWORD`, or
/// the local OBS websocket config. The returned source is safe to log.
pub fn load_websocket_password(explicit: Option<&str>) -> (Option<String>, WsCredentials) {
    if let Some(password) = explicit {
        return (Some(password.to_owned()), WsCredentials::Explicit);
    }

    if let Ok(password) = std::env::var("OBS_WS_PASSWORD") {
        if !password.is_empty() {
            return (Some(password), WsCredentials::Environment);
        }
    }

    if let Some(path) = detect_installation().websocket_config {
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<WebsocketConfig>(&raw) {
                if config.auth_required && config.server_password.is_some() {
                    return (config.server_password, WsCredentials::ObsConfig);
                }
            }
        }
    }

    (None, WsCredentials::None)
}

#[derive(Debug, Deserialize)]
struct WebsocketConfig {
    #[serde(default)]
    auth_required: bool,
    server_password: Option<String>,
}

/// Check whether a local TCP port accepts connections.
pub fn port_reachable(host: &str, port: u16) -> bool {
    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok() {
            return true;
        }
    }
    false
}

/// A required or optional OBS plugin.
#[derive(Clone, Debug, Serialize)]
pub struct PluginStatus {
    pub key: String,
    pub name: String,
    pub required: bool,
    pub installed: bool,
    pub evidence: Vec<String>,
    pub install_url: String,
    pub note: Option<String>,
}

impl PluginStatus {
    pub fn mark(&mut self, evidence: impl Into<String>) {
        self.installed = true;
        self.evidence.push(evidence.into());
    }
}

fn plugin_statuses(installation: &ObsInstallation) -> Vec<PluginStatus> {
    let dirs = installation.all_plugin_dirs();
    let specs = [
        (
            "move_transition",
            "Move Transition",
            "move-transition",
            "https://github.com/exeldro/obs-move-transition/releases",
            None,
        ),
        (
            "advanced_masks",
            "Advanced Masks",
            "advanced-masks",
            "https://github.com/FiniteSingularity/obs-advanced-masks/releases",
            Some("OBS 30-32 needs the 'legacy' Windows package."),
        ),
        (
            "aitum_vertical",
            "Aitum Vertical",
            "vertical-canvas",
            "https://github.com/Aitum/obs-vertical-canvas/releases",
            None,
        ),
        (
            "background_removal",
            "Background Removal",
            "backgroundremoval",
            "https://github.com/royshil/obs-backgroundremoval/releases",
            None,
        ),
        (
            "multi_rtmp",
            "Multi RTMP",
            "multi-rtmp",
            "https://github.com/sorayuki/obs-multi-rtmp/releases",
            None,
        ),
    ];

    specs
        .into_iter()
        .map(|(key, name, marker, url, note)| {
            let mut status = PluginStatus {
                key: key.to_owned(),
                name: name.to_owned(),
                required: true,
                installed: false,
                evidence: Vec::new(),
                install_url: url.to_owned(),
                note: note.map(str::to_owned),
            };

            let files = find_marker_files(&dirs, marker);
            for file in files.into_iter().take(4) {
                status.mark(format!("file: {}", file.display()));
            }
            status
        })
        .collect()
}

fn find_marker_files(dirs: &[PathBuf], marker: &str) -> Vec<PathBuf> {
    let marker = marker.to_ascii_lowercase();
    let mut found = Vec::new();
    for dir in dirs {
        scan_dir(dir, &marker, 0, &mut found);
    }
    found
}

fn scan_dir(dir: &Path, marker: &str, depth: u8, found: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if name.contains(marker) {
            found.push(path.clone());
        }
        if file_type.is_dir() {
            scan_dir(&path, marker, depth + 1, found);
        }
    }
}

/// A canvas exposed by OBS (the main canvas plus plugin canvases such as
/// Aitum Vertical).
#[derive(Clone, Debug, Serialize)]
pub struct CanvasInfo {
    pub name: String,
    pub uuid: String,
    pub width: u32,
    pub height: u32,
    pub is_main: bool,
}

/// Runtime information retrieved over obs-websocket.
#[derive(Clone, Debug, Serialize)]
pub struct ObsRuntime {
    pub obs_studio_version: String,
    pub obs_websocket_version: String,
    pub rpc_version: u32,
    pub platform: String,
    pub platform_description: String,
    pub input_kind_count: usize,
    pub filter_kind_count: usize,
    pub transition_kind_count: usize,
    pub input_kinds: Vec<String>,
    pub filter_kinds: Vec<String>,
    pub transition_kinds: Vec<String>,
    pub canvas_list: Vec<CanvasInfo>,
    pub transition_instances: Vec<String>,
    pub move_transition_instance: Option<String>,
}

/// Full Phase 0 report.
#[derive(Clone, Debug, Serialize)]
pub struct DiscoveryReport {
    pub installation: ObsInstallation,
    pub websocket_credentials: WsCredentials,
    pub websocket_reachable: bool,
    pub runtime: ObsRuntime,
    pub plugins: Vec<PluginStatus>,
    pub warnings: Vec<String>,
    pub ready: bool,
    pub missing_plugins: Vec<String>,
}

/// Connect to OBS, inspect versions and plugin kinds, and return a report.
pub async fn discover(
    host: &str,
    port: u16,
    password: Option<&str>,
    credentials: WsCredentials,
) -> Result<DiscoveryReport> {
    let installation = detect_installation();
    let reachable = port_reachable(host, port);
    if !reachable {
        anyhow::bail!(
            "obs-websocket is not reachable at {host}:{port}. Open OBS and enable Tools -> WebSocket Server Settings."
        );
    }

    let client = tokio::time::timeout(
        Duration::from_secs(10),
        Client::connect(host, port, password),
    )
    .await
    .context("timed out connecting to obs-websocket")?
    .context("failed to connect to obs-websocket")?;

    let version = client
        .general()
        .version()
        .await
        .context("GetVersion failed")?;
    let input_kinds = client
        .inputs()
        .list_kinds(false)
        .await
        .context("GetInputKindList failed")?;
    let filter_kinds = client
        .filters()
        .list_kinds()
        .await
        .context("GetSourceFilterKindList failed")?;
    let transition_kinds = client
        .transitions()
        .list_kinds()
        .await
        .context("GetTransitionKindList failed")?;
    let transition_list = client
        .transitions()
        .list()
        .await
        .context("GetSceneTransitionList failed")?;
    let canvas_list = client
        .canvases()
        .list()
        .await
        .context("GetCanvasList failed")?
        .into_iter()
        .map(|canvas| CanvasInfo {
            name: canvas.id.name,
            uuid: canvas.id.uuid.to_string(),
            width: canvas.video_settings.base_width,
            height: canvas.video_settings.base_height,
            is_main: canvas
                .flags
                .contains(obws::responses::canvases::CanvasFlags::MAIN),
        })
        .collect::<Vec<_>>();

    let transition_instances = transition_list
        .transitions
        .iter()
        .map(|transition| transition.id.name.clone())
        .collect::<Vec<_>>();
    let move_transition_instance = transition_list
        .transitions
        .iter()
        .find(|transition| transition.kind == "move_transition")
        .map(|transition| transition.id.name.clone());

    let mut plugins = plugin_statuses(&installation);
    let mut aitum_version = None;
    match client
        .general()
        .call_vendor_request::<Value, Value>(CallVendorRequest {
            vendor_name: "aitum-vertical-canvas",
            request_type: "version",
            request_data: &Value::Object(Default::default()),
        })
        .await
    {
        Ok(response) => {
            aitum_version = response
                .response_data
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        Err(err) => tracing::debug!(%err, "Aitum Vertical vendor version request failed"),
    }

    for plugin in &mut plugins {
        let api_evidence = match plugin.key.as_str() {
            "move_transition" => filter_kinds
                .iter()
                .find(|kind| {
                    kind.as_str() == "move_value_filter" || kind.as_str() == "move_source_filter"
                })
                .map(|kind| format!("filter kind: {kind}")),
            "advanced_masks" => filter_kinds
                .iter()
                .find(|kind| {
                    kind.as_str() == "advanced_masks_filter"
                        || kind.as_str() == "advanced_masks_filter_v2"
                })
                .map(|kind| format!("filter kind: {kind}")),
            "aitum_vertical" => aitum_version
                .as_deref()
                .map(|version| format!("vendor version: {version}")),
            "background_removal" => filter_kinds
                .iter()
                .find(|kind| kind.as_str() == "background_removal")
                .map(|kind| format!("filter kind: {kind}")),
            _ => None,
        };
        if let Some(evidence) = api_evidence {
            plugin.mark(evidence);
        }
    }

    let missing_plugins = plugins
        .iter()
        .filter(|plugin| plugin.required && !plugin.installed)
        .map(|plugin| plugin.name.clone())
        .collect::<Vec<_>>();

    let mut warnings = Vec::new();
    if !transition_kinds
        .iter()
        .any(|kind| kind == "move_transition")
    {
        warnings.push(
            "The Move Transition plugin is not exposing the `move_transition` transition kind."
                .to_owned(),
        );
    } else if move_transition_instance.is_none() {
        warnings.push(
            "Move Transition is installed, but no `Move` transition instance exists yet. \
             Add one in OBS (Scene Transitions -> +) or the 350 ms animated switch will use the \
             configured fallback."
                .to_owned(),
        );
    }
    if !canvas_list.iter().any(|canvas| !canvas.is_main) {
        warnings.push(
            "No secondary canvas was reported. Aitum Vertical may not have initialised a 9:16 canvas."
                .to_owned(),
        );
    }

    let runtime = ObsRuntime {
        obs_studio_version: version.obs_studio_version.to_string(),
        obs_websocket_version: version.obs_web_socket_version.to_string(),
        rpc_version: version.rpc_version,
        platform: version.platform,
        platform_description: version.platform_description,
        input_kind_count: input_kinds.len(),
        filter_kind_count: filter_kinds.len(),
        transition_kind_count: transition_kinds.len(),
        input_kinds,
        filter_kinds,
        transition_kinds,
        canvas_list,
        transition_instances,
        move_transition_instance,
    };

    Ok(DiscoveryReport {
        installation,
        websocket_credentials: credentials,
        websocket_reachable: reachable,
        runtime,
        plugins,
        warnings,
        ready: missing_plugins.is_empty(),
        missing_plugins,
    })
}

/// Human-readable plugin installation help for the current OS.
pub fn install_help(installation: &ObsInstallation) -> String {
    let plugin_dir = installation
        .user_plugin_dirs
        .first()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "your OBS user plugin directory".to_owned());

    format!(
        "Install each missing plugin from its release page, then restart OBS.\n\
         Recommended per-user plugin location: {plugin_dir}\n\
         Use the Windows ZIP for OBS 30-32 where available, or run the Windows installer.\n\
         Do not install 32-bit packages into a 64-bit OBS."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_has_expected_os() {
        let installation = detect_installation();
        assert_eq!(installation.os, std::env::consts::OS);
    }

    #[test]
    fn plugin_scan_does_not_panic_on_missing_dirs() {
        let installation = ObsInstallation {
            os: "test",
            binary: None,
            system_plugin_dirs: vec![PathBuf::from("Z:/definitely/not/here")],
            user_plugin_dirs: vec![],
            websocket_config: None,
        };
        let statuses = plugin_statuses(&installation);
        assert_eq!(statuses.len(), 5);
        assert!(statuses.iter().all(|status| !status.installed));
    }
}
