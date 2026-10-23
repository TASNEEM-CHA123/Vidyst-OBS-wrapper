use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use obws::{
    common::MonitorType,
    events::Event,
    requests::{
        filters::{Create as CreateFilter, SetEnabled as SetFilterEnabled},
        inputs::{Create as CreateInput, InputId, Volume},
        scene_items::SetEnabled as SetSceneItemEnabled,
        scenes::SceneId,
        sources::{SourceId, TakeScreenshot},
        EventSubscription,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::{
    sync::{Notify, RwLock},
    task::JoinHandle,
};
use vidyst_layouts::{
    builtin_layout, builtin_layouts, AspectRatio, Layout, SourceId as LayoutSource,
};
use vidyst_obs_driver::{
    bootstrap::{
        current_vertical_scene, run_bootstrap, set_mask_radius, switch_vertical_scene,
        trigger_morph, vertical_scene_uuid, BootstrapOptions, BootstrapReport,
        CAMERA_DEFAULT_FRAME_INTERVAL, CAMERA_DEFAULT_RESOLUTION, CAMERA_GROUP, CAMERA_RAW,
        CIRCLE_RADIUS, MIC_SOURCE, MORPH_TO_CIRCLE, MORPH_TO_TILE, SCREEN_AUDIO_SOURCE,
        SCREEN_SOURCE, TILE_RADIUS, TRANSITION_DURATION_MS,
    },
    discovery::{self, DiscoveryReport, WsCredentials},
    load_websocket_password,
};

use crate::settings::{self, PersistedSettings};

const EVENT_STATUS: &str = "obs-status";
const EVENT_STUDIO: &str = "obs-event";
const EVENT_PREVIEW_FRAME: &str = "obs-preview-frame";
const EVENT_PREVIEW_ERROR: &str = "obs-preview-error";
const EVENT_AUDIO_METERS: &str = "audio-meters";
const BACKGROUND_FILTER: &str = "Vidyst_Background_Removal";
const BACKGROUND_FILTER_KIND: &str = "background_removal";
const WINDOW_SOURCE: &str = "Vidyst_Window";
const WINDOW_INPUT_KIND: &str = "window_capture";
const BACKGROUND_SOURCE: &str = "Vidyst_Background";
const BACKGROUND_INPUT_KIND: &str = "color_source_v3";
const BACKGROUND_IMAGE_SOURCE: &str = "Vidyst_Background_Image";
const BACKGROUND_IMAGE_INPUT_KIND: &str = "image_source";
const VERTICAL_VENDOR: &str = "aitum-vertical-canvas";
const PREVIEW_FPS: u64 = 15;
const HORIZONTAL_PREVIEW_WIDTH: u32 = 960;
const HORIZONTAL_PREVIEW_HEIGHT: u32 = 540;
const VERTICAL_PREVIEW_WIDTH: u32 = 540;
const VERTICAL_PREVIEW_HEIGHT: u32 = 960;
const VERTICAL_WIDTH: u32 = 1080;
const VERTICAL_HEIGHT: u32 = 1920;
const HORIZONTAL_WIDTH: f32 = 1920.0;
const HORIZONTAL_HEIGHT: f32 = 1080.0;

#[derive(Clone, Debug)]
struct ObsConfig {
    host: String,
    port: u16,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectionStatus {
    pub state: String,
    pub message: String,
    pub connected: bool,
    pub obs_version: Option<String>,
    pub websocket_version: Option<String>,
    pub rpc_version: Option<u32>,
    pub reconnect_attempt: u32,
    pub bootstrap_ready: bool,
}

impl ConnectionStatus {
    fn connecting(attempt: u32, host: &str, port: u16) -> Self {
        Self {
            state: "connecting".to_owned(),
            message: format!("Connecting to OBS at {host}:{port}…"),
            connected: false,
            obs_version: None,
            websocket_version: None,
            rpc_version: None,
            reconnect_attempt: attempt,
            bootstrap_ready: false,
        }
    }

    fn error(attempt: u32, message: String) -> Self {
        Self {
            state: "error".to_owned(),
            message,
            connected: false,
            obs_version: None,
            websocket_version: None,
            rpc_version: None,
            reconnect_attempt: attempt,
            bootstrap_ready: false,
        }
    }

    fn disconnected(message: String) -> Self {
        Self {
            state: "disconnected".to_owned(),
            message,
            connected: false,
            obs_version: None,
            websocket_version: None,
            rpc_version: None,
            reconnect_attempt: 0,
            bootstrap_ready: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StudioEvent {
    pub kind: String,
    pub detail: Value,
    pub at_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AudioChannel {
    pub input: String,
    pub label: String,
    pub muted: bool,
    pub monitoring: bool,
    pub monitor_type: String,
    pub volume_db: f32,
    pub volume_mul: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceSelection {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceOption {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceLists {
    pub cameras: Vec<DeviceOption>,
    pub screens: Vec<DeviceOption>,
    pub windows: Vec<DeviceOption>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StudioSnapshot {
    pub connected: bool,
    pub bootstrap_ready: bool,
    pub obs_version: Option<String>,
    pub websocket_version: Option<String>,
    pub program_scene: Option<String>,
    pub vertical_scene: Option<String>,
    pub active_layout: Option<String>,
    pub active_layout_name: Option<String>,
    pub active_vertical_layout: Option<String>,
    pub transition: Option<String>,
    pub transition_duration_ms: u64,
    pub recording: bool,
    pub recording_horizontal: bool,
    pub recording_vertical: bool,
    pub recording_paused: bool,
    pub recording_timecode: String,
    pub recording_directory: Option<String>,
    pub recording_format: String,
    pub last_recording_path: Option<String>,
    pub streaming: bool,
    pub streaming_horizontal: bool,
    pub streaming_vertical: bool,
    pub stream_reconnecting: bool,
    pub stream_timecode: String,
    pub camera: Option<DeviceSelection>,
    pub camera_connected: bool,
    pub screen: Option<DeviceSelection>,
    pub screen_connected: bool,
    pub screen_mode: String,
    pub window: Option<DeviceSelection>,
    pub camera_visible: bool,
    pub screen_visible: bool,
    pub camera_radius: f64,
    pub background_removal: bool,
    pub background_mode: String,
    pub background_color: String,
    pub background_image: Option<String>,
    pub audio: Vec<AudioChannel>,
    pub stream_key_configured: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreviewFrame {
    pub target: String,
    pub image: String,
    pub width: u32,
    pub height: u32,
    pub at_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AudioMeter {
    pub input: String,
    pub peak_db: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct VerticalOutputStatus {
    #[serde(default)]
    recording: bool,
    #[serde(default)]
    streaming: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QuickResizeRequest {
    pub corner: Option<String>,
    pub size_pct: Option<f32>,
    pub shape: Option<String>,
    pub split_pct: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetCamera {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radius_pct: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutPreset {
    pub name: String,
    pub layout_id: String,
    pub camera: Option<PresetCamera>,
    pub split_pct: Option<f32>,
    pub created_at_unix: u64,
}

pub struct ObsService {
    app: AppHandle,
    settings: RwLock<PersistedSettings>,
    config: RwLock<ObsConfig>,
    password: RwLock<Option<String>>,
    client: RwLock<Option<Arc<Client>>>,
    bootstrap: RwLock<Option<BootstrapReport>>,
    status: RwLock<ConnectionStatus>,
    event_task: RwLock<Option<JoinHandle<()>>>,
    last_recording_path: Mutex<Option<String>>,
    vertical_record_started: Mutex<Option<Instant>>,
    vertical_stream_started: Mutex<Option<Instant>>,
    generation: AtomicU64,
    preview_active: AtomicBool,
    preview_horizontal: AtomicBool,
    preview_vertical: AtomicBool,
    last_meter_emit: AtomicU64,
    reconnect: Notify,
    lost: Notify,
}

impl ObsService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        let settings = settings::load(&app);
        let (password, _) = load_websocket_password(None);
        let service = Arc::new(Self {
            app,
            config: RwLock::new(ObsConfig {
                host: settings.obs_host.clone(),
                port: settings.obs_port,
            }),
            settings: RwLock::new(settings),
            password: RwLock::new(password),
            client: RwLock::new(None),
            bootstrap: RwLock::new(None),
            status: RwLock::new(ConnectionStatus::disconnected(
                "Waiting for the OBS connection…".to_owned(),
            )),
            event_task: RwLock::new(None),
            last_recording_path: Mutex::new(None),
            vertical_record_started: Mutex::new(None),
            vertical_stream_started: Mutex::new(None),
            generation: AtomicU64::new(0),
            preview_active: AtomicBool::new(false),
            preview_horizontal: AtomicBool::new(true),
            preview_vertical: AtomicBool::new(true),
            last_meter_emit: AtomicU64::new(0),
            reconnect: Notify::new(),
            lost: Notify::new(),
        });
        service
    }

    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            service.manager_loop().await;
        });
        let preview = self.clone();
        tauri::async_runtime::spawn(async move {
            preview.preview_loop().await;
        });
    }

    async fn set_status(&self, status: ConnectionStatus) {
        *self.status.write().await = status.clone();
        let _ = self.app.emit(EVENT_STATUS, status);
    }

    pub async fn status(&self) -> ConnectionStatus {
        self.status.read().await.clone()
    }

    fn emit_event(&self, kind: impl Into<String>, detail: Value) {
        let event = StudioEvent {
            kind: kind.into(),
            detail,
            at_ms: now_ms(),
        };
        let _ = self.app.emit(EVENT_STUDIO, event);
    }

    async fn manager_loop(self: Arc<Self>) {
        let mut attempt = 0_u32;
        loop {
            let generation = self.generation.load(Ordering::SeqCst);
            let config = self.config.read().await.clone();
            let password = self.password.read().await.clone();
            self.set_status(ConnectionStatus::connecting(
                attempt,
                &config.host,
                config.port,
            ))
            .await;

            let connect = tokio::time::timeout(
                Duration::from_secs(10),
                Client::connect(&config.host, config.port, password.as_deref()),
            )
            .await;

            match connect {
                Ok(Ok(client)) => {
                    if generation != self.generation.load(Ordering::SeqCst) {
                        continue;
                    }
                    let client = Arc::new(client);
                    // The default subscription excludes high-volume events. Add
                    // volume meters explicitly; everything else keeps the
                    // standard non-high-volume subscription.
                    let _ = client
                        .reidentify(EventSubscription::ALL | EventSubscription::INPUT_VOLUME_METERS)
                        .await;
                    let version = client.general().version().await.ok();
                    let options = self.bootstrap_options().await;
                    let bootstrap = run_bootstrap(&client, &options).await;

                    if generation != self.generation.load(Ordering::SeqCst) {
                        continue;
                    }

                    if let Ok(report) = &bootstrap {
                        self.persist_bootstrap_devices(report).await;
                    }
                    *self.client.write().await = Some(client.clone());
                    *self.bootstrap.write().await = bootstrap.as_ref().ok().cloned();
                    if bootstrap.is_ok() {
                        let _ = sync_camera_shape_for_active_layout(&client).await;
                        let _ = self.sync_persisted_visuals(&client).await;
                    }
                    attempt = 0;

                    let (obs_version, websocket_version, rpc_version) = match &version {
                        Some(v) => (
                            Some(v.obs_studio_version.to_string()),
                            Some(v.obs_web_socket_version.to_string()),
                            Some(v.rpc_version),
                        ),
                        None => (None, None, None),
                    };

                    let status = match &bootstrap {
                        Ok(report) => {
                            self.emit_event(
                                "bootstrap",
                                json!({
                                    "scenes": report.scenes.len(),
                                    "requests": report.requests.len(),
                                    "failed": report.requests.iter().filter(|r| !r.ok).count(),
                                }),
                            );
                            ConnectionStatus {
                                state: "connected".to_owned(),
                                message: "OBS connected. Scenes and sources are ready.".to_owned(),
                                connected: true,
                                obs_version,
                                websocket_version,
                                rpc_version,
                                reconnect_attempt: 0,
                                bootstrap_ready: true,
                            }
                        }
                        Err(error) => ConnectionStatus {
                            state: "connected".to_owned(),
                            message: format!(
                                "OBS connected, but scene setup needs attention: {error:#}"
                            ),
                            connected: true,
                            obs_version,
                            websocket_version,
                            rpc_version,
                            reconnect_attempt: 0,
                            bootstrap_ready: false,
                        },
                    };
                    self.set_status(status).await;

                    let service = self.clone();
                    let event_client = client.clone();
                    let task = tokio::spawn(async move {
                        service.event_loop(event_client, generation).await;
                    });
                    *self.event_task.write().await = Some(task);

                    tokio::select! {
                        _ = self.reconnect.notified() => {
                            attempt = 0;
                        }
                        _ = self.lost.notified() => {
                            attempt = attempt.saturating_add(1);
                        }
                    }

                    if let Some(task) = self.event_task.write().await.take() {
                        task.abort();
                    }
                    *self.client.write().await = None;
                    *self.bootstrap.write().await = None;
                }
                Ok(Err(error)) => {
                    if generation != self.generation.load(Ordering::SeqCst) {
                        continue;
                    }
                    attempt = attempt.saturating_add(1);
                    self.set_status(ConnectionStatus::error(
                        attempt,
                        format!("{error}. Retrying automatically."),
                    ))
                    .await;
                    self.wait_backoff(attempt).await;
                }
                Err(_) => {
                    if generation != self.generation.load(Ordering::SeqCst) {
                        continue;
                    }
                    attempt = attempt.saturating_add(1);
                    self.set_status(ConnectionStatus::error(
                        attempt,
                        "Timed out connecting to OBS. Retrying automatically.".to_owned(),
                    ))
                    .await;
                    self.wait_backoff(attempt).await;
                }
            }
        }
    }

    pub fn set_preview_active(&self, active: bool, horizontal: bool, vertical: bool) {
        self.preview_active.store(active, Ordering::Relaxed);
        self.preview_horizontal.store(horizontal, Ordering::Relaxed);
        self.preview_vertical.store(vertical, Ordering::Relaxed);
    }

    async fn preview_loop(self: Arc<Self>) {
        let interval = Duration::from_millis(1000 / PREVIEW_FPS);
        let mut vertical_retry_at: Option<Instant> = None;
        let mut last_vertical_error: Option<String> = None;
        loop {
            if !self.preview_active.load(Ordering::Relaxed) || self.client.read().await.is_none() {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
            let started = Instant::now();
            let capture_horizontal = self.preview_horizontal.load(Ordering::Relaxed);
            let capture_vertical = self.preview_vertical.load(Ordering::Relaxed)
                && vertical_retry_at.is_none_or(|retry| Instant::now() >= retry);

            if capture_horizontal {
                if let Ok(frame) = self.preview_frame("horizontal").await {
                    let _ = self.app.emit(EVENT_PREVIEW_FRAME, frame);
                }
            }
            if capture_vertical {
                match self.preview_frame("vertical").await {
                    Ok(frame) => {
                        vertical_retry_at = None;
                        last_vertical_error = None;
                        let _ = self.app.emit(EVENT_PREVIEW_FRAME, frame);
                    }
                    Err(error) => {
                        let message = format!("{error:#}");
                        if last_vertical_error.as_deref() != Some(&message) {
                            let _ = self.app.emit(
                                EVENT_PREVIEW_ERROR,
                                json!({ "target": "vertical", "message": message }),
                            );
                            last_vertical_error = Some(message);
                        }
                        vertical_retry_at = Some(Instant::now() + Duration::from_secs(2));
                    }
                }
            }

            let elapsed = started.elapsed();
            if elapsed < interval {
                tokio::time::sleep(interval - elapsed).await;
            }
        }
    }

    async fn preview_frame(&self, target: &str) -> Result<PreviewFrame> {
        let client = self.client().await?;
        let (scene, width, height, source_uuid) = if target == "vertical" {
            let vertical = current_vertical_scene(&client).await?;
            let scene = vertical
                .get("scene")
                .and_then(Value::as_str)
                .context("Aitum did not report a current vertical scene")?;
            (
                scene.to_owned(),
                VERTICAL_PREVIEW_WIDTH,
                VERTICAL_PREVIEW_HEIGHT,
                Some(vertical_scene_uuid(&client, scene).await?),
            )
        } else {
            (
                current_program_scene_name(&client).await?,
                HORIZONTAL_PREVIEW_WIDTH,
                HORIZONTAL_PREVIEW_HEIGHT,
                None,
            )
        };
        let image = if let Some(source_uuid) = source_uuid {
            // Scene names are shared between the main and vertical canvases.
            // Using the UUID keeps the vertical preview on the Aitum canvas
            // instead of capturing the same-named horizontal scene.
            let response = client
                .raw_request(
                    "GetSourceScreenshot",
                    json!({
                        "sourceUuid": source_uuid,
                        "imageFormat": "jpg",
                        "imageWidth": width,
                        "imageHeight": height,
                        "imageCompressionQuality": 75,
                    }),
                )
                .await
                .with_context(|| format!("capture the live {target} preview"))?;
            response
                .get("imageData")
                .and_then(Value::as_str)
                .context("OBS did not return image data for the vertical preview")?
                .to_owned()
        } else {
            client
                .sources()
                .take_screenshot(TakeScreenshot {
                    source: SourceId::Name(&scene),
                    format: "jpg",
                    width: Some(width),
                    height: Some(height),
                    compression_quality: Some(75),
                })
                .await
                .with_context(|| format!("capture the live {target} preview"))?
        };
        Ok(PreviewFrame {
            target: target.to_owned(),
            image,
            width,
            height,
            at_ms: now_ms(),
        })
    }

    async fn wait_backoff(&self, attempt: u32) {
        let seconds = 1_u64 << attempt.min(3);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(seconds)) => {}
            _ = self.reconnect.notified() => {}
        }
    }

    async fn event_loop(&self, client: Arc<Client>, generation: u64) {
        let mut stream = match client.events() {
            Ok(stream) => stream,
            Err(error) => {
                if generation == self.generation.load(Ordering::SeqCst) {
                    *self.client.write().await = None;
                    self.set_status(ConnectionStatus::error(
                        0,
                        format!("OBS event subscription failed: {error}"),
                    ))
                    .await;
                    self.lost.notify_one();
                }
                return;
            }
        };

        while let Some(event) = stream.next().await {
            if generation != self.generation.load(Ordering::SeqCst) {
                return;
            }
            self.handle_event(event);
        }

        if generation == self.generation.load(Ordering::SeqCst) {
            *self.client.write().await = None;
            self.set_status(ConnectionStatus::disconnected(
                "The OBS WebSocket connection closed. Reconnecting…".to_owned(),
            ))
            .await;
            self.lost.notify_one();
        }
    }

    fn handle_event(&self, event: Event) {
        let (kind, detail) = match event {
            Event::InputVolumeMeters { inputs } => {
                let now = now_ms();
                let last = self.last_meter_emit.load(Ordering::Relaxed);
                if now.saturating_sub(last) < 40 {
                    return;
                }
                self.last_meter_emit.store(now, Ordering::Relaxed);
                let meters: Vec<AudioMeter> = inputs
                    .iter()
                    .filter(|input| input.name == MIC_SOURCE || input.name == SCREEN_AUDIO_SOURCE)
                    .map(|input| {
                        let peak = input
                            .levels
                            .iter()
                            .filter_map(|levels| levels.first().copied())
                            .fold(0.0_f32, f32::max);
                        AudioMeter {
                            input: input.name.clone(),
                            peak_db: if peak > 0.000_01 {
                                (20.0 * peak.log10()).clamp(-60.0, 0.0)
                            } else {
                                -60.0
                            },
                        }
                    })
                    .collect();
                let _ = self.app.emit(
                    EVENT_AUDIO_METERS,
                    json!({ "meters": meters, "at_ms": now }),
                );
                return;
            }
            Event::CurrentProgramSceneChanged { id } => {
                ("program_scene", json!({ "scene": id.name }))
            }
            Event::CurrentSceneTransitionChanged { id } => {
                ("transition", json!({ "transition": id.name }))
            }
            Event::CurrentSceneTransitionDurationChanged { duration } => (
                "transition_duration",
                json!({ "duration_ms": duration.whole_milliseconds().max(0) as u64 }),
            ),
            Event::SceneTransitionStarted { id } => {
                ("transition_started", json!({ "transition": id.name }))
            }
            Event::SceneTransitionEnded { id } => {
                ("transition_ended", json!({ "transition": id.name }))
            }
            Event::StreamStateChanged { active, state } => (
                "stream",
                json!({ "active": active, "state": format!("{state:?}") }),
            ),
            Event::RecordStateChanged {
                active,
                state,
                path,
            } => {
                if !active {
                    if let Some(path) = path.as_ref() {
                        if let Ok(mut last) = self.last_recording_path.lock() {
                            *last = Some(path.clone());
                        }
                    }
                }
                (
                    "record",
                    json!({ "active": active, "state": format!("{state:?}"), "path": path }),
                )
            }
            Event::RecordFileChanged { path } => {
                if let Ok(mut last) = self.last_recording_path.lock() {
                    *last = Some(path.clone());
                }
                ("record_file", json!({ "path": path }))
            }
            Event::InputMuteStateChanged { id, muted } => {
                ("mute", json!({ "input": id.name, "muted": muted }))
            }
            Event::InputVolumeChanged { id, mul, db } => {
                ("volume", json!({ "input": id.name, "mul": mul, "db": db }))
            }
            Event::InputAudioMonitorTypeChanged { id, monitor_type } => (
                "monitor",
                json!({ "input": id.name, "monitorType": monitor_type_name(monitor_type) }),
            ),
            Event::SceneItemEnableStateChanged {
                scene,
                item_id,
                enabled,
            } => (
                "visibility",
                json!({
                    "scene": scene.name,
                    "sceneItemId": item_id,
                    "enabled": enabled,
                }),
            ),
            Event::SceneItemTransformChanged {
                scene,
                item_id,
                transform,
            } => (
                "transform",
                json!({
                    "scene": scene.name,
                    "sceneItemId": item_id,
                    "transform": {
                        "x": transform.position_x,
                        "y": transform.position_y,
                        "width": transform.bounds_width,
                        "height": transform.bounds_height,
                    },
                }),
            ),
            Event::SourceFilterEnableStateChanged {
                source,
                filter,
                enabled,
            } => (
                "filter",
                json!({
                    "source": source,
                    "filter": filter,
                    "enabled": enabled,
                }),
            ),
            Event::InputSettingsChanged { id, settings } => (
                "input_settings",
                json!({ "input": id.name, "settings": settings }),
            ),
            Event::VendorEvent {
                vendor_name,
                event_type,
                event_data,
            } if vendor_name == VERTICAL_VENDOR => (
                "vertical_output",
                json!({ "event": event_type, "data": event_data }),
            ),
            Event::ExitStarted => ("obs_exit", json!({})),
            _ => return,
        };
        self.emit_event(kind, detail);
    }

    async fn client(&self) -> Result<Arc<Client>> {
        self.client
            .read()
            .await
            .clone()
            .context("OBS is not connected yet. Check the connection in Settings.")
    }

    async fn bootstrap_options(&self) -> BootstrapOptions {
        let settings = self.settings.read().await.clone();
        BootstrapOptions {
            camera_device: settings.camera_device_value,
            monitor: settings.screen_device_value,
            mic_device: None,
            screen_audio_device: None,
            camera_connected: settings.camera_connected,
            screen_connected: settings.screen_connected,
        }
    }

    async fn persist_bootstrap_devices(&self, report: &BootstrapReport) {
        let mut settings = self.settings.write().await;
        let mut changed = false;
        if let (Some(value), Some(name)) = (
            report.camera.device_value.as_ref(),
            report.camera.device_name.as_ref(),
        ) {
            if settings.camera_device_value.as_deref() != Some(value) {
                settings.camera_device_value = Some(value.clone());
                changed = true;
            }
            if settings.camera_device_name.as_deref() != Some(name) {
                settings.camera_device_name = Some(name.clone());
                changed = true;
            }
        }
        if let (Some(value), Some(name)) = (
            report.screen.device_value.as_ref(),
            report.screen.device_name.as_ref(),
        ) {
            if settings.screen_device_value.as_deref() != Some(value) {
                settings.screen_device_value = Some(value.clone());
                changed = true;
            }
            if settings.screen_device_name.as_deref() != Some(name) {
                settings.screen_device_name = Some(name.clone());
                changed = true;
            }
        }
        if changed {
            let _ = settings::save(&self.app, &settings);
        }
    }

    async fn ensure_bootstrap(&self) -> Result<BootstrapReport> {
        if let Some(report) = self.bootstrap.read().await.clone() {
            return Ok(report);
        }
        let client = self.client().await?;
        let options = self.bootstrap_options().await;
        let report = run_bootstrap(&client, &options).await?;
        self.persist_bootstrap_devices(&report).await;
        *self.bootstrap.write().await = Some(report.clone());
        Ok(report)
    }

    pub async fn reconnect(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(task) = self.event_task.write().await.take() {
            task.abort();
        }
        *self.client.write().await = None;
        *self.bootstrap.write().await = None;
        let config = self.config.read().await.clone();
        self.set_status(ConnectionStatus::connecting(0, &config.host, config.port))
            .await;
        self.reconnect.notify_one();
    }

    pub async fn update_obs_config(
        &self,
        host: String,
        port: u16,
        password: Option<String>,
    ) -> Result<()> {
        let host = host.trim().to_owned();
        if host.is_empty() {
            bail!("OBS host cannot be empty");
        }
        if port == 0 {
            bail!("OBS WebSocket port must be between 1 and 65535");
        }

        {
            let mut settings = self.settings.write().await;
            settings.obs_host = host.clone();
            settings.obs_port = port;
            settings::save(&self.app, &settings)?;
        }
        *self.config.write().await = ObsConfig { host, port };
        if let Some(password) = password {
            *self.password.write().await = if password.is_empty() {
                None
            } else {
                Some(password)
            };
        }
        self.reconnect().await;
        Ok(())
    }

    pub async fn update_recording_config(
        &self,
        path: Option<String>,
        format: String,
    ) -> Result<()> {
        let format = match format.as_str() {
            "hybrid_mp4" | "mkv" | "mp4" => format,
            _ => bail!("unsupported recording format"),
        };
        {
            let mut settings = self.settings.write().await;
            settings.record_path = path.filter(|value| !value.trim().is_empty());
            settings.record_format = format.clone();
            settings::save(&self.app, &settings)?;
        }

        if let Ok(client) = self.client().await {
            if let Some(path) = self.settings.read().await.record_path.clone() {
                let _ = client
                    .raw_request("SetRecordDirectory", json!({ "recordDirectory": path }))
                    .await;
            }
            let _ = apply_recording_format(&client, &format).await;
        }
        Ok(())
    }

    pub async fn set_record_directory(&self, path: String) -> Result<StudioSnapshot> {
        let path = path.trim().to_owned();
        if path.is_empty() {
            bail!("recording folder cannot be empty");
        }
        if !Path::new(&path).is_dir() {
            bail!("recording folder does not exist: {path}");
        }
        let client = self.client().await?;
        client
            .config()
            .set_record_directory(&path)
            .await
            .context("set the OBS recording folder")?;
        {
            let mut settings = self.settings.write().await;
            settings.record_path = Some(path.clone());
            settings::save(&self.app, &settings)?;
        }
        self.emit_event("recording_directory", json!({ "path": path }));
        self.snapshot().await
    }

    pub async fn update_stream_config(
        &self,
        server: Option<String>,
        key: Option<String>,
    ) -> Result<()> {
        if let Some(key) = key {
            let key = key.trim().to_owned();
            if key.is_empty() {
                settings::clear_stream_key()?;
            } else {
                settings::write_stream_key(&key)?;
            }
        }
        let mut settings = self.settings.write().await;
        settings.stream_server = server.filter(|value| !value.trim().is_empty());
        settings::save(&self.app, &settings)?;
        Ok(())
    }

    pub async fn settings_view(&self) -> settings::SettingsView {
        let settings = self.settings.read().await.clone();
        let password = self.password.read().await.clone();
        settings::view(&self.app, password.as_deref(), &settings)
    }

    pub async fn plugin_health(&self) -> Result<DiscoveryReport> {
        let config = self.config.read().await.clone();
        let password = self.password.read().await.clone();
        let credentials = if password.is_some() {
            WsCredentials::ObsConfig
        } else {
            WsCredentials::None
        };
        discovery::discover(&config.host, config.port, password.as_deref(), credentials).await
    }

    pub async fn snapshot(&self) -> Result<StudioSnapshot> {
        let client = self.client().await?;
        let status = self.status().await;
        let report = self.bootstrap.read().await.clone();
        let program_scene = current_program_scene_name(&client).await.ok();
        let vertical_scene = current_vertical_scene(&client)
            .await
            .ok()
            .and_then(|value| {
                value
                    .get("scene")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });

        let active_layout = program_scene
            .as_deref()
            .and_then(layout_id_from_scene_name)
            .or_else(|| {
                vertical_scene
                    .as_deref()
                    .and_then(layout_id_from_scene_name)
            });
        let active_layout_name = active_layout
            .as_deref()
            .and_then(builtin_layout)
            .map(|layout| layout.name);
        let active_vertical_layout = vertical_scene
            .as_deref()
            .and_then(layout_id_from_scene_name);

        let audio = audio_channels(&client).await;
        let recording = client.recording().status().await.ok();
        let streaming = client.streaming().status().await.ok();
        let vertical = vertical_output_status(&client).await.unwrap_or_default();
        let transition = client.transitions().current().await.ok();

        let settings = self.settings.read().await.clone();
        let camera_live = input_setting_string(&client, CAMERA_RAW, "video_device_id").await;
        let screen_live = if settings.screen_mode == "window" {
            input_setting_string(&client, WINDOW_SOURCE, "window").await
        } else {
            input_setting_string(&client, SCREEN_SOURCE, "monitor_id").await
        };
        let camera_connected = camera_live.as_ref().is_some_and(|value| !value.is_empty());
        let screen_connected = screen_live.as_ref().is_some_and(|value| !value.is_empty());
        let recording_directory = client
            .config()
            .record_directory()
            .await
            .ok()
            .or_else(|| settings.record_path.clone());
        let last_recording_path = self
            .last_recording_path
            .lock()
            .ok()
            .and_then(|path| path.clone());
        let screen_source = if settings.screen_mode == "window" {
            WINDOW_SOURCE
        } else {
            SCREEN_SOURCE
        };
        let (camera_visible, screen_visible) = match &program_scene {
            Some(scene) => source_visibility(&client, scene, screen_source).await,
            None => (false, false),
        };
        let camera_radius = current_mask_radius(&client).await.unwrap_or(TILE_RADIUS);
        let background_removal = background_removal_enabled(&client).await;
        let window = settings.window_value.as_ref().map(|value| DeviceSelection {
            name: settings
                .window_name
                .clone()
                .unwrap_or_else(|| "Application window".to_owned()),
            value: value.clone(),
        });

        let camera = settings
            .camera_device_value
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(|value| DeviceSelection {
                name: settings
                    .camera_device_name
                    .clone()
                    .unwrap_or_else(|| "Camera".to_owned()),
                value: value.clone(),
            })
            .or_else(|| {
                report.as_ref().and_then(|report| {
                    report
                        .camera
                        .device_value
                        .as_ref()
                        .map(|value| DeviceSelection {
                            name: report
                                .camera
                                .device_name
                                .clone()
                                .unwrap_or_else(|| "Camera".to_owned()),
                            value: value.clone(),
                        })
                })
            });
        let screen = settings
            .screen_device_value
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(|value| DeviceSelection {
                name: settings
                    .screen_device_name
                    .clone()
                    .unwrap_or_else(|| "Display".to_owned()),
                value: value.clone(),
            })
            .or_else(|| {
                report.as_ref().and_then(|report| {
                    report
                        .screen
                        .device_value
                        .as_ref()
                        .map(|value| DeviceSelection {
                            name: report
                                .screen
                                .device_name
                                .clone()
                                .unwrap_or_else(|| "Display".to_owned()),
                            value: value.clone(),
                        })
                })
            });
        let recording_horizontal = recording.as_ref().is_some_and(|value| value.active);
        let recording_vertical = vertical.recording;
        let recording_timecode = if recording_horizontal {
            recording
                .as_ref()
                .map(|value| format_duration(value.duration))
                .unwrap_or_else(|| "00:00:00".to_owned())
        } else {
            vertical_elapsed(&self.vertical_record_started, recording_vertical)
        };
        let streaming_horizontal = streaming.as_ref().is_some_and(|value| value.active);
        let streaming_vertical = vertical.streaming;
        let stream_timecode = if streaming_horizontal {
            streaming
                .as_ref()
                .map(|value| format_duration(value.duration))
                .unwrap_or_else(|| "00:00:00".to_owned())
        } else {
            vertical_elapsed(&self.vertical_stream_started, streaming_vertical)
        };

        Ok(StudioSnapshot {
            connected: status.connected,
            bootstrap_ready: status.bootstrap_ready,
            obs_version: status.obs_version,
            websocket_version: status.websocket_version,
            program_scene,
            vertical_scene,
            active_layout,
            active_layout_name,
            active_vertical_layout,
            transition: transition.as_ref().map(|value| value.id.name.clone()),
            transition_duration_ms: transition
                .as_ref()
                .and_then(|value| value.duration)
                .map(|duration| duration.whole_milliseconds().max(0) as u64)
                .unwrap_or(TRANSITION_DURATION_MS),
            recording: recording_horizontal || recording_vertical,
            recording_horizontal,
            recording_vertical,
            recording_paused: recording.as_ref().is_some_and(|value| value.paused),
            recording_timecode,
            recording_directory,
            recording_format: settings.record_format.clone(),
            last_recording_path,
            streaming: streaming_horizontal || streaming_vertical,
            streaming_horizontal,
            streaming_vertical,
            stream_reconnecting: streaming.as_ref().is_some_and(|value| value.reconnecting),
            stream_timecode,
            camera,
            camera_connected,
            screen,
            screen_connected,
            screen_mode: settings.screen_mode,
            window,
            camera_visible,
            screen_visible,
            camera_radius,
            background_removal,
            background_mode: settings.background_mode,
            background_color: settings.background_color,
            background_image: settings.background_image,
            audio,
            stream_key_configured: settings::read_stream_key().is_some(),
        })
    }

    pub async fn layouts(&self) -> Vec<Layout> {
        builtin_layouts().to_vec()
    }

    pub async fn prepare_scenes(&self) -> Result<StudioSnapshot> {
        let client = self.client().await?;
        let options = self.bootstrap_options().await;
        let report = run_bootstrap(&client, &options).await?;
        self.persist_bootstrap_devices(&report).await;
        *self.bootstrap.write().await = Some(report.clone());
        let _ = sync_camera_shape_for_active_layout(&client).await;
        self.emit_event(
            "bootstrap",
            json!({
                "scenes": report.scenes.len(),
                "requests": report.requests.len(),
                "failed": report.requests.iter().filter(|request| !request.ok).count(),
            }),
        );
        let mut status = self.status().await;
        status.state = "connected".to_owned();
        status.connected = true;
        status.bootstrap_ready = true;
        status.message = "OBS connected. Scenes and sources are ready.".to_owned();
        status.reconnect_attempt = 0;
        self.set_status(status).await;

        let _ = self.sync_persisted_visuals(&client).await;
        self.snapshot().await
    }

    /// Re-apply source and background choices that are stored in app settings.
    /// This runs on every reconnect so OBS matches the UI immediately.
    async fn sync_persisted_visuals(&self, client: &Client) -> Result<()> {
        let settings = self.settings.read().await.clone();
        if settings.screen_connected
            && settings.screen_mode == "window"
            && self.ensure_window_source(client).await.is_ok()
        {
            let _ = self.sync_screen_mode(client, true).await;
        }
        match settings.background_mode.as_str() {
            "color" => {
                if self.ensure_background_source(client).await.is_ok() {
                    let _ = self.sync_background(client, "color").await;
                }
            }
            "image" => {
                if let Some(image) = settings.background_image.as_deref() {
                    if Path::new(image).is_file()
                        && self
                            .ensure_background_image_source(client, image)
                            .await
                            .is_ok()
                    {
                        let _ = self.sync_background(client, "image").await;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn apply_layout(
        &self,
        id: String,
        horizontal: bool,
        vertical: bool,
    ) -> Result<StudioSnapshot> {
        if !horizontal && !vertical {
            bail!("select at least one output canvas");
        }
        let layout = builtin_layout(&id).with_context(|| format!("unknown layout {id}"))?;
        let report = self.ensure_bootstrap().await?;
        let scene = report
            .scene(&id)
            .with_context(|| format!("layout {id} has not been prepared in OBS"))?;
        let client = self.client().await?;

        if horizontal {
            client
                .scenes()
                .set_current_program_scene(SceneId::Name(&scene.name))
                .await
                .context("switch the horizontal program scene")?;
        }
        if vertical {
            let response = switch_vertical_scene(&client, &scene.name).await?;
            if response.get("success").and_then(Value::as_bool) == Some(false) {
                bail!("Aitum rejected the vertical scene switch: {response}");
            }
        }

        let ratio = if horizontal {
            AspectRatio::R16x9
        } else {
            AspectRatio::R9x16
        };
        let shape = layout
            .slots_for(ratio)
            .and_then(|slots| {
                slots
                    .iter()
                    .find(|slot| slot.source == LayoutSource::Camera)
            })
            .map(|slot| slot.radius_pct)
            .unwrap_or(TILE_RADIUS as f32);
        if shape >= 50.0 {
            trigger_morph(&client, MORPH_TO_CIRCLE).await?;
        } else {
            trigger_morph(&client, MORPH_TO_TILE).await?;
        }

        let settings = self.settings.read().await.clone();
        if settings.screen_mode == "window" {
            let _ = self.sync_screen_mode(&client, true).await;
        }
        if settings.background_mode == "color" || settings.background_mode == "image" {
            let _ = self
                .sync_background(&client, &settings.background_mode)
                .await;
        }

        self.snapshot().await
    }

    pub async fn quick_resize(&self, request: QuickResizeRequest) -> Result<StudioSnapshot> {
        let report = self.ensure_bootstrap().await?;
        let client = self.client().await?;
        let scene_name = current_program_scene_name(&client)
            .await
            .context("read the current program scene")?;
        let scene = report
            .scenes
            .iter()
            .find(|scene| scene.name == scene_name)
            .with_context(|| {
                format!("quick resize is available in a Vidyst layout, not {scene_name}")
            })?;
        if scene.id == "just_me" || scene.id == "screen_focus" {
            bail!("the camera is not adjustable in this layout");
        }

        let mut camera_transform = client
            .scene_items()
            .transform(SceneId::Name(&scene.name), scene.main_camera.item_id)
            .await
            .context("read the camera transform")?;

        if let Some(split_pct) = request.split_pct {
            if scene.id == "side_by_side" || scene.id == "side_by_side_inv" {
                let split = (split_pct / 100.0).clamp(0.35, 0.85);
                let (screen_x, camera_x) = if scene.id == "side_by_side" {
                    (0.02, 0.02 + split + 0.02)
                } else {
                    (0.02 + (0.94 - split) + 0.02, 0.02)
                };
                set_transform(
                    &client,
                    &scene.name,
                    scene.main_screen.item_id,
                    screen_x,
                    0.03,
                    split,
                    0.94,
                    "OBS_BOUNDS_SCALE_INNER",
                    false,
                )
                .await?;
                set_transform(
                    &client,
                    &scene.name,
                    scene.main_camera.item_id,
                    camera_x,
                    0.03,
                    0.94 - split,
                    0.94,
                    "OBS_BOUNDS_SCALE_OUTER",
                    true,
                )
                .await?;
                camera_transform = client
                    .scene_items()
                    .transform(SceneId::Name(&scene.name), scene.main_camera.item_id)
                    .await
                    .context("read the camera transform after the split")?;
            }
        }

        let current_width = if camera_transform.bounds_width > 1.0 {
            camera_transform.bounds_width
        } else {
            camera_transform.width.max(1.0)
        };
        let current_height = if camera_transform.bounds_height > 1.0 {
            camera_transform.bounds_height
        } else {
            camera_transform.height.max(1.0)
        };
        let aspect = (current_width / current_height).max(0.1);

        let mut width = current_width;
        let mut height = current_height;
        if let Some(size_pct) = request.size_pct {
            height = HORIZONTAL_HEIGHT * (size_pct.clamp(8.0, 75.0) / 100.0);
            width = height * aspect;
        }
        if request
            .shape
            .as_deref()
            .is_some_and(|shape| shape == "circle")
        {
            width = width.min(height);
            height = width;
        }
        width = width.min(HORIZONTAL_WIDTH * 0.9);
        height = height.min(HORIZONTAL_HEIGHT * 0.9);

        let margin = 0.02;
        let (mut x, mut y) = (
            camera_transform.position_x / HORIZONTAL_WIDTH,
            camera_transform.position_y / HORIZONTAL_HEIGHT,
        );
        match request.corner.as_deref() {
            Some("tl") => {
                x = margin;
                y = margin;
            }
            Some("tr") => {
                x = 0.98 - width / HORIZONTAL_WIDTH;
                y = margin;
            }
            Some("bl") => {
                x = margin;
                y = 0.98 - height / HORIZONTAL_HEIGHT;
            }
            Some("br") => {
                x = 0.98 - width / HORIZONTAL_WIDTH;
                y = 0.98 - height / HORIZONTAL_HEIGHT;
            }
            _ => {
                x = x.clamp(margin, (0.98 - width / HORIZONTAL_WIDTH).max(margin));
                y = y.clamp(margin, (0.98 - height / HORIZONTAL_HEIGHT).max(margin));
            }
        }

        set_transform_pixels(
            &client,
            &scene.name,
            scene.main_camera.item_id,
            x * HORIZONTAL_WIDTH,
            y * HORIZONTAL_HEIGHT,
            width,
            height,
            "OBS_BOUNDS_SCALE_OUTER",
            true,
        )
        .await?;

        if let Some(shape) = request.shape.as_deref() {
            match shape {
                "circle" => {
                    set_mask_radius(&client, CIRCLE_RADIUS).await?;
                    trigger_morph(&client, MORPH_TO_CIRCLE).await?;
                }
                "tile" => {
                    set_mask_radius(&client, TILE_RADIUS).await?;
                    trigger_morph(&client, MORPH_TO_TILE).await?;
                }
                _ => {}
            }
        }

        self.snapshot().await
    }

    pub async fn set_audio_mute(&self, input: String, muted: bool) -> Result<()> {
        validate_audio_input(&input)?;
        let client = self.client().await?;
        client
            .inputs()
            .set_muted(InputId::Name(&input), muted)
            .await
            .with_context(|| format!("set mute for {input}"))
    }

    pub async fn set_audio_monitor(&self, input: String, monitoring: bool) -> Result<()> {
        validate_audio_input(&input)?;
        let client = self.client().await?;
        client
            .inputs()
            .set_audio_monitor_type(
                InputId::Name(&input),
                if monitoring {
                    MonitorType::MonitorAndOutput
                } else {
                    MonitorType::None
                },
            )
            .await
            .with_context(|| format!("set audio monitoring for {input}"))
    }

    pub async fn set_audio_volume(&self, input: String, db: f32) -> Result<()> {
        validate_audio_input(&input)?;
        if !db.is_finite() {
            bail!("volume must be a finite number");
        }
        let client = self.client().await?;
        client
            .inputs()
            .set_volume(InputId::Name(&input), Volume::Db(db.clamp(-60.0, 10.0)))
            .await
            .with_context(|| format!("set volume for {input}"))
    }

    pub async fn toggle_background_removal(&self, enabled: bool) -> Result<StudioSnapshot> {
        let client = self.client().await?;
        let filters = client
            .filters()
            .list(SourceId::Name(CAMERA_RAW))
            .await
            .context("list camera filters")?;
        if !filters
            .iter()
            .any(|filter| filter.name == BACKGROUND_FILTER)
        {
            client
                .filters()
                .create(CreateFilter {
                    source: SourceId::Name(CAMERA_RAW),
                    filter: BACKGROUND_FILTER,
                    kind: BACKGROUND_FILTER_KIND,
                    settings: Some(json!({})),
                })
                .await
                .context("create the background removal filter")?;
        }
        client
            .filters()
            .set_enabled(SetFilterEnabled {
                source: SourceId::Name(CAMERA_RAW),
                filter: BACKGROUND_FILTER,
                enabled,
            })
            .await
            .context("toggle background removal")?;
        self.snapshot().await
    }

    pub async fn list_devices(&self) -> Result<DeviceLists> {
        let client = self.client().await?;
        let cameras = client
            .inputs()
            .properties_list_property_items(InputId::Name(CAMERA_RAW), "video_device_id")
            .await
            .context("list camera devices")?
            .into_iter()
            .map(device_option)
            .collect();
        let screens = client
            .inputs()
            .properties_list_property_items(InputId::Name(SCREEN_SOURCE), "monitor_id")
            .await
            .context("list displays")?
            .into_iter()
            .map(device_option)
            .collect();
        let windows = if self.ensure_window_source(&client).await.is_ok() {
            client
                .inputs()
                .properties_list_property_items(InputId::Name(WINDOW_SOURCE), "window")
                .await
                .unwrap_or_default()
                .into_iter()
                .map(device_option)
                .filter(|option| !option.value.is_empty())
                .collect()
        } else {
            Vec::new()
        };
        Ok(DeviceLists {
            cameras,
            screens,
            windows,
        })
    }

    async fn ensure_window_source(&self, client: &Client) -> Result<()> {
        if input_exists(client, WINDOW_SOURCE).await? {
            return Ok(());
        }
        let report = self
            .bootstrap
            .read()
            .await
            .clone()
            .context("prepare the Vidyst scenes before adding a window source")?;
        let target = report
            .scenes
            .iter()
            .find(|scene| scene.id == "just_me")
            .or_else(|| report.scenes.first())
            .with_context(|| "no Vidyst scene is available for the window source")?;
        client
            .inputs()
            .create(CreateInput {
                scene: SceneId::Name(&target.name),
                input: WINDOW_SOURCE,
                kind: WINDOW_INPUT_KIND,
                settings: Some(json!({})),
                enabled: Some(false),
            })
            .await
            .context("create the application-window capture source")?;
        Ok(())
    }

    async fn ensure_background_source(&self, client: &Client) -> Result<()> {
        let color = {
            let settings = self.settings.read().await;
            obs_color(&settings.background_color)?
        };
        let source_settings = json!({
            "color": color,
            "width": HORIZONTAL_WIDTH as u32,
            "height": HORIZONTAL_HEIGHT as u32,
        });
        self.ensure_background_input(
            client,
            BACKGROUND_SOURCE,
            BACKGROUND_INPUT_KIND,
            source_settings.clone(),
        )
        .await?;
        // Keep an existing OBS source in sync with the persisted color too,
        // including after the color byte order was corrected.
        client
            .inputs()
            .set_settings(obws::requests::inputs::SetSettings {
                input: InputId::Name(BACKGROUND_SOURCE),
                settings: &source_settings,
                overlay: Some(true),
            })
            .await
            .context("sync background color")?;
        Ok(())
    }

    async fn ensure_background_image_source(&self, client: &Client, image: &str) -> Result<()> {
        if !input_exists(client, BACKGROUND_IMAGE_SOURCE).await? {
            self.ensure_background_input(
                client,
                BACKGROUND_IMAGE_SOURCE,
                BACKGROUND_IMAGE_INPUT_KIND,
                json!({
                    "file": image,
                    "unload": false,
                }),
            )
            .await?;
        }
        // Give a freshly-created image source a beat to load before pointing it
        // at the requested file.
        tokio::time::sleep(Duration::from_millis(100)).await;
        client
            .inputs()
            .set_settings(obws::requests::inputs::SetSettings {
                input: InputId::Name(BACKGROUND_IMAGE_SOURCE),
                settings: &json!({ "file": image, "unload": false }),
                overlay: Some(true),
            })
            .await
            .context("set background image")?;
        Ok(())
    }

    async fn ensure_background_input(
        &self,
        client: &Client,
        name: &str,
        kind: &str,
        input_settings: Value,
    ) -> Result<()> {
        if input_exists(client, name).await? {
            return Ok(());
        }
        let report = self
            .bootstrap
            .read()
            .await
            .clone()
            .context("prepare the Vidyst scenes before adding a background")?;
        let target = report
            .scenes
            .iter()
            .find(|scene| scene.id == "just_me")
            .or_else(|| report.scenes.first())
            .with_context(|| "no Vidyst scene is available for the background source")?;
        let created = client
            .inputs()
            .create(CreateInput {
                scene: SceneId::Name(&target.name),
                input: name,
                kind,
                settings: Some(input_settings.clone()),
                enabled: Some(false),
            })
            .await;
        // Older scene collections may expose the unversioned color source.
        if created.is_err() && name == BACKGROUND_SOURCE {
            client
                .inputs()
                .create(CreateInput {
                    scene: SceneId::Name(&target.name),
                    input: name,
                    kind: "color_source",
                    settings: Some(input_settings),
                    enabled: Some(false),
                })
                .await
                .context("create the background color source")?;
        } else {
            created.with_context(|| format!("create the {name} background source"))?;
        }
        Ok(())
    }

    async fn sync_screen_mode(&self, client: &Client, show_window: bool) -> Result<()> {
        let report = self
            .bootstrap
            .read()
            .await
            .clone()
            .context("Vidyst scenes are not ready")?;
        for scene in &report.scenes {
            sync_scene_window(
                client,
                &scene.main_scene_uuid,
                scene.main_screen.enabled,
                show_window,
            )
            .await?;
            // The Aitum canvas is best-effort: a window source may not be
            // cloneable into a vendor canvas on every OBS build.
            let _ = sync_scene_window(
                client,
                &scene.vertical_scene_uuid,
                scene.vertical_screen.enabled,
                show_window,
            )
            .await;
        }
        Ok(())
    }

    async fn sync_background(&self, client: &Client, mode: &str) -> Result<()> {
        let report = self
            .bootstrap
            .read()
            .await
            .clone()
            .context("Vidyst scenes are not ready")?;
        let color_exists = input_exists(client, BACKGROUND_SOURCE).await?;
        let image_exists = input_exists(client, BACKGROUND_IMAGE_SOURCE).await?;
        for scene in &report.scenes {
            if color_exists || mode == "color" {
                let _ = sync_scene_background(
                    client,
                    &scene.main_scene_uuid,
                    BACKGROUND_SOURCE,
                    HORIZONTAL_WIDTH,
                    HORIZONTAL_HEIGHT,
                    mode == "color",
                    true,
                )
                .await;
                let _ = sync_scene_background(
                    client,
                    &scene.vertical_scene_uuid,
                    BACKGROUND_SOURCE,
                    HORIZONTAL_HEIGHT,
                    HORIZONTAL_WIDTH,
                    mode == "color",
                    true,
                )
                .await;
            }
            if image_exists || mode == "image" {
                let _ = sync_scene_background(
                    client,
                    &scene.main_scene_uuid,
                    BACKGROUND_IMAGE_SOURCE,
                    HORIZONTAL_WIDTH,
                    HORIZONTAL_HEIGHT,
                    mode == "image",
                    false,
                )
                .await;
                let _ = sync_scene_background(
                    client,
                    &scene.vertical_scene_uuid,
                    BACKGROUND_IMAGE_SOURCE,
                    HORIZONTAL_HEIGHT,
                    HORIZONTAL_WIDTH,
                    mode == "image",
                    false,
                )
                .await;
            }
        }
        Ok(())
    }

    pub async fn set_camera_source(&self, value: String) -> Result<StudioSnapshot> {
        let value = value.trim().to_owned();
        if value.is_empty() {
            bail!("choose a camera device");
        }
        self.ensure_bootstrap().await?;
        let client = self.client().await?;
        let name = property_display_name(&client, CAMERA_RAW, "video_device_id", &value)
            .await
            .unwrap_or_else(|| "Camera".to_owned());
        client
            .inputs()
            .set_settings(obws::requests::inputs::SetSettings {
                input: InputId::Name(CAMERA_RAW),
                settings: &json!({
                    "video_device_id": value,
                    "resolution": CAMERA_DEFAULT_RESOLUTION,
                    "frame_interval": CAMERA_DEFAULT_FRAME_INTERVAL,
                    "video_format": 0,
                }),
                overlay: Some(true),
            })
            .await
            .context("switch camera device")?;
        {
            let mut settings = self.settings.write().await;
            settings.camera_connected = true;
            settings.camera_device_value = Some(value);
            settings.camera_device_name = Some(name);
            settings::save(&self.app, &settings)?;
        }
        // Give the DirectShow source a moment to initialize before reading it back.
        tokio::time::sleep(Duration::from_millis(500)).await;
        self.snapshot().await
    }

    pub async fn set_camera_connected(&self, connected: bool) -> Result<StudioSnapshot> {
        self.ensure_bootstrap().await?;
        let client = self.client().await?;
        let current_value = input_setting_string(&client, CAMERA_RAW, "video_device_id").await;
        if connected {
            let saved = self.settings.read().await.clone();
            let value = saved
                .camera_device_value
                .clone()
                .filter(|value| !value.is_empty())
                .or_else(|| current_value.filter(|value| !value.is_empty()));
            let value = match value {
                Some(value) => value,
                None => first_property_value(&client, CAMERA_RAW, "video_device_id")
                    .await
                    .context("OBS did not report an available camera device")?,
            };
            let name = property_display_name(&client, CAMERA_RAW, "video_device_id", &value)
                .await
                .or(saved.camera_device_name)
                .unwrap_or_else(|| "Camera".to_owned());
            client
                .inputs()
                .set_settings(obws::requests::inputs::SetSettings {
                    input: InputId::Name(CAMERA_RAW),
                    settings: &json!({
                        "video_device_id": value,
                        "resolution": CAMERA_DEFAULT_RESOLUTION,
                        "frame_interval": CAMERA_DEFAULT_FRAME_INTERVAL,
                        "video_format": 0,
                    }),
                    overlay: Some(true),
                })
                .await
                .context("connect the camera")?;
            {
                let mut settings = self.settings.write().await;
                settings.camera_connected = true;
                settings.camera_device_value = Some(value);
                settings.camera_device_name = Some(name);
                settings::save(&self.app, &settings)?;
            }
            tokio::time::sleep(Duration::from_millis(600)).await;
        } else {
            let fallback_name = self.settings.read().await.camera_device_name.clone();
            let name = match current_value.as_deref().filter(|value| !value.is_empty()) {
                Some(value) => property_display_name(&client, CAMERA_RAW, "video_device_id", value)
                    .await
                    .or(fallback_name),
                None => None,
            };
            client
                .inputs()
                .set_settings(obws::requests::inputs::SetSettings {
                    input: InputId::Name(CAMERA_RAW),
                    settings: &json!({ "video_device_id": "" }),
                    overlay: Some(true),
                })
                .await
                .context("disconnect the camera")?;
            {
                let mut settings = self.settings.write().await;
                settings.camera_connected = false;
                if let Some(value) = current_value.filter(|value| !value.is_empty()) {
                    settings.camera_device_value = Some(value);
                }
                if let Some(name) = name {
                    settings.camera_device_name = Some(name);
                }
                settings::save(&self.app, &settings)?;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        self.emit_event(
            "source_connection",
            json!({ "source": "camera", "connected": connected }),
        );
        self.snapshot().await
    }

    pub async fn set_screen_source(&self, value: String) -> Result<StudioSnapshot> {
        let value = value.trim().to_owned();
        if value.is_empty() {
            bail!("choose a display");
        }
        self.ensure_bootstrap().await?;
        let client = self.client().await?;
        let name = property_display_name(&client, SCREEN_SOURCE, "monitor_id", &value)
            .await
            .unwrap_or_else(|| "Display".to_owned());
        client
            .inputs()
            .set_settings(obws::requests::inputs::SetSettings {
                input: InputId::Name(SCREEN_SOURCE),
                settings: &json!({ "monitor_id": value }),
                overlay: Some(true),
            })
            .await
            .context("switch display source")?;
        {
            let mut settings = self.settings.write().await;
            settings.screen_mode = "display".to_owned();
            settings.screen_connected = true;
            settings.screen_device_value = Some(value);
            settings.screen_device_name = Some(name);
            settings::save(&self.app, &settings)?;
        }
        self.sync_screen_mode(&client, false).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.snapshot().await
    }

    pub async fn set_screen_connected(&self, connected: bool) -> Result<StudioSnapshot> {
        self.ensure_bootstrap().await?;
        let client = self.client().await?;
        if connected {
            let saved = self.settings.read().await.clone();
            if saved.screen_mode == "window" {
                let value = saved
                    .window_value
                    .clone()
                    .filter(|value| !value.is_empty())
                    .context("choose an application window before connecting")?;
                self.ensure_window_source(&client).await?;
                client
                    .inputs()
                    .set_settings(obws::requests::inputs::SetSettings {
                        input: InputId::Name(WINDOW_SOURCE),
                        settings: &json!({ "window": value }),
                        overlay: Some(true),
                    })
                    .await
                    .context("connect the application window")?;
                self.sync_screen_mode(&client, true).await?;
                let mut settings = self.settings.write().await;
                settings.screen_connected = true;
                settings::save(&self.app, &settings)?;
            } else {
                let value = saved
                    .screen_device_value
                    .clone()
                    .filter(|value| !value.is_empty());
                let value = match value {
                    Some(value) => value,
                    None => first_property_value(&client, SCREEN_SOURCE, "monitor_id")
                        .await
                        .context("OBS did not report an available display")?,
                };
                let name = property_display_name(&client, SCREEN_SOURCE, "monitor_id", &value)
                    .await
                    .or(saved.screen_device_name)
                    .unwrap_or_else(|| "Display".to_owned());
                client
                    .inputs()
                    .set_settings(obws::requests::inputs::SetSettings {
                        input: InputId::Name(SCREEN_SOURCE),
                        settings: &json!({ "monitor_id": value }),
                        overlay: Some(true),
                    })
                    .await
                    .context("connect the display")?;
                self.sync_screen_mode(&client, false).await?;
                let mut settings = self.settings.write().await;
                settings.screen_connected = true;
                settings.screen_device_value = Some(value);
                settings.screen_device_name = Some(name);
                settings::save(&self.app, &settings)?;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        } else {
            let settings_snapshot = self.settings.read().await.clone();
            if settings_snapshot.screen_mode == "window" {
                if input_exists(&client, WINDOW_SOURCE).await? {
                    client
                        .inputs()
                        .set_settings(obws::requests::inputs::SetSettings {
                            input: InputId::Name(WINDOW_SOURCE),
                            settings: &json!({ "window": "" }),
                            overlay: Some(true),
                        })
                        .await
                        .context("disconnect the application window")?;
                }
            } else {
                client
                    .inputs()
                    .set_settings(obws::requests::inputs::SetSettings {
                        input: InputId::Name(SCREEN_SOURCE),
                        settings: &json!({ "monitor_id": "" }),
                        overlay: Some(true),
                    })
                    .await
                    .context("disconnect the display")?;
            }
            {
                let mut settings = self.settings.write().await;
                settings.screen_connected = false;
                settings::save(&self.app, &settings)?;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        self.emit_event(
            "source_connection",
            json!({ "source": "screen", "connected": connected }),
        );
        self.snapshot().await
    }

    pub async fn set_window_source(&self, value: String, name: String) -> Result<StudioSnapshot> {
        self.ensure_bootstrap().await?;
        let client = self.client().await?;
        self.ensure_window_source(&client).await?;
        client
            .inputs()
            .set_settings(obws::requests::inputs::SetSettings {
                input: InputId::Name(WINDOW_SOURCE),
                settings: &json!({ "window": value }),
                overlay: Some(true),
            })
            .await
            .context("switch application-window source")?;
        {
            let mut settings = self.settings.write().await;
            settings.screen_mode = "window".to_owned();
            settings.screen_connected = true;
            settings.window_value = Some(value);
            settings.window_name = Some(name);
            settings::save(&self.app, &settings)?;
        }
        self.sync_screen_mode(&client, true).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.snapshot().await
    }

    pub async fn set_background(
        &self,
        mode: String,
        color: String,
        image: Option<String>,
    ) -> Result<StudioSnapshot> {
        if mode != "none" && mode != "color" && mode != "image" {
            bail!("background mode must be none, color, or image");
        }
        let color = normalize_hex_color(&color)?;
        self.ensure_bootstrap().await?;
        let client = self.client().await?;

        let image = image
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if mode == "image" {
            let path = image
                .as_deref()
                .context("choose a background image before enabling image mode")?;
            if !Path::new(path).is_file() {
                bail!("the selected background image does not exist: {path}");
            }
            self.ensure_background_image_source(&client, path).await?;
        }
        if mode == "color" {
            self.ensure_background_source(&client).await?;
            // A newly-created color source loads its defaults asynchronously;
            // give OBS a beat before applying the requested color.
            tokio::time::sleep(Duration::from_millis(100)).await;
            let value = obs_color(&color)?;
            client
                .inputs()
                .set_settings(obws::requests::inputs::SetSettings {
                    input: InputId::Name(BACKGROUND_SOURCE),
                    settings: &json!({
                        "color": value,
                        "width": HORIZONTAL_WIDTH as u32,
                        "height": HORIZONTAL_HEIGHT as u32,
                    }),
                    overlay: Some(true),
                })
                .await
                .context("set background color")?;
        }
        {
            let mut settings = self.settings.write().await;
            settings.background_mode = mode.clone();
            settings.background_color = color;
            if image.is_some() {
                settings.background_image = image;
            }
            settings::save(&self.app, &settings)?;
        }
        let color_exists = input_exists(&client, BACKGROUND_SOURCE).await?;
        let image_exists = input_exists(&client, BACKGROUND_IMAGE_SOURCE).await?;
        if mode != "none" || color_exists || image_exists {
            self.sync_background(&client, &mode).await?;
        }
        self.snapshot().await
    }

    pub async fn set_source_visibility(
        &self,
        source: String,
        visible: bool,
    ) -> Result<StudioSnapshot> {
        if source != CAMERA_GROUP && source != SCREEN_SOURCE && source != WINDOW_SOURCE {
            bail!("unknown source");
        }
        let client = self.client().await?;
        let scene = current_program_scene_name(&client)
            .await
            .context("read the current program scene")?;
        let items = client
            .scene_items()
            .list(SceneId::Name(&scene))
            .await
            .context("list scene items")?;
        let item = items
            .iter()
            .find(|item| item.source_name == source)
            .with_context(|| format!("{source} is not present in this layout"))?;
        client
            .scene_items()
            .set_enabled(SetSceneItemEnabled {
                scene: SceneId::Name(&scene),
                item_id: item.id,
                enabled: visible,
            })
            .await
            .with_context(|| format!("set visibility for {source}"))?;
        self.snapshot().await
    }

    pub async fn start_recording(&self, horizontal: bool, vertical: bool) -> Result<()> {
        if !horizontal && !vertical {
            bail!("select at least one output canvas before recording");
        }
        let client = self.client().await?;
        let settings = self.settings.read().await.clone();
        let mut main_started = false;

        if horizontal {
            if !client.recording().status().await?.active {
                if let Some(path) = settings.record_path.as_deref() {
                    let _ = client
                        .raw_request("SetRecordDirectory", json!({ "recordDirectory": path }))
                        .await;
                }
                let _ = apply_recording_format(&client, &settings.record_format).await;
                client
                    .recording()
                    .start()
                    .await
                    .context("start the 16:9 recording")?;
                main_started = true;
                if !wait_for_main_recording(&client, true).await {
                    let _ = client.recording().stop().await;
                    bail!("The 16:9 recording did not start. Check the OBS recording settings.");
                }
            }
        }

        if vertical {
            if let Err(error) = sync_vertical_scene_to_program(&client).await {
                if main_started {
                    let _ = client.recording().stop().await;
                }
                return Err(error);
            }
            let status = vertical_output_status(&client).await.unwrap_or_default();
            if !status.recording {
                if let Err(error) = call_vertical_output(&client, "start_recording").await {
                    if main_started {
                        let _ = client.recording().stop().await;
                    }
                    return Err(error);
                }
                if !wait_for_vertical(&client, Some(true), None).await {
                    let _ = call_vertical_output(&client, "stop_recording").await;
                    if main_started {
                        let _ = client.recording().stop().await;
                    }
                    bail!(
                        "Aitum vertical recording did not start. Open the Aitum Vertical dock and check its recording settings."
                    );
                }
            }
            if let Ok(mut started) = self.vertical_record_started.lock() {
                if started.is_none() {
                    *started = Some(Instant::now());
                }
            }
        }

        Ok(())
    }

    pub async fn stop_recording(&self) -> Result<()> {
        let client = self.client().await?;
        let mut errors = Vec::new();
        if client.recording().status().await?.active {
            if let Err(error) = client.recording().stop().await {
                errors.push(format!("16:9 recording: {error:#}"));
            }
        }
        let vertical = vertical_output_status(&client).await.unwrap_or_default();
        if vertical.recording {
            if let Err(error) = call_vertical_output(&client, "stop_recording").await {
                errors.push(format!("9:16 recording: {error:#}"));
            } else if !wait_for_vertical(&client, Some(false), None).await {
                errors.push("Aitum vertical recording did not stop.".to_owned());
            }
            self.capture_latest_vertical_recording().await;
        }
        if let Ok(mut started) = self.vertical_record_started.lock() {
            *started = None;
        }
        if errors.is_empty() {
            Ok(())
        } else {
            bail!("{}", errors.join(" "));
        }
    }

    async fn capture_latest_vertical_recording(&self) {
        let directory = self.settings.read().await.record_path.clone().or_else(|| {
            std::env::var("USERPROFILE")
                .ok()
                .map(|path| format!("{path}\\Videos"))
        });
        let Some(directory) = directory else {
            return;
        };
        if let Some(path) = newest_recording(Path::new(&directory)) {
            if let Ok(mut last) = self.last_recording_path.lock() {
                *last = Some(path.to_string_lossy().into_owned());
            }
        }
    }

    pub async fn split_recording(&self) -> Result<()> {
        let client = self.client().await?;
        let horizontal_active = client.recording().status().await?.active;
        let vertical_active = vertical_output_status(&client)
            .await
            .unwrap_or_default()
            .recording;
        if !horizontal_active && !vertical_active {
            bail!("recording is not active");
        }
        if horizontal_active {
            if client.recording().split_file().await.is_err() {
                // Hybrid MP4 rejects SplitRecordFile on some OBS builds.
                // Close and reopen the output so the user still gets a file
                // boundary instead of a failed split button.
                let settings = self.settings.read().await.clone();
                client
                    .recording()
                    .stop()
                    .await
                    .context("close the 16:9 recording for the split")?;
                if !wait_for_main_recording(&client, false).await {
                    bail!("The 16:9 recording did not stop for the split.");
                }
                if let Some(path) = settings.record_path.as_deref() {
                    let _ = client
                        .raw_request("SetRecordDirectory", json!({ "recordDirectory": path }))
                        .await;
                }
                let _ = apply_recording_format(&client, &settings.record_format).await;
                client
                    .recording()
                    .start()
                    .await
                    .context("start the next 16:9 recording after the split")?;
                if !wait_for_main_recording(&client, true).await {
                    bail!("The next 16:9 recording did not start after the split.");
                }
            }
        }
        if vertical_active {
            // Aitum does not expose its split-file command. Stop and restart
            // the independent vertical recording to continue in a new file.
            call_vertical_output(&client, "stop_recording")
                .await
                .context("close the current 9:16 recording")?;
            if !wait_for_vertical(&client, Some(false), None).await {
                bail!("Aitum did not stop the 9:16 recording for the split");
            }
            call_vertical_output(&client, "start_recording")
                .await
                .context("start the next 9:16 recording after the split")?;
            if !wait_for_vertical(&client, Some(true), None).await {
                bail!("Aitum did not start the next 9:16 recording after the split");
            }
        }
        Ok(())
    }

    pub async fn start_streaming(&self, horizontal: bool, vertical: bool) -> Result<()> {
        if !horizontal && !vertical {
            bail!("select at least one output canvas before going live");
        }
        let client = self.client().await?;
        // Keep the Aitum vertical canvas on the same layout as the horizontal
        // program before any output starts. Multi-RTMP destinations are started
        // by the plugin's own Sync Start setting when OBS goes live.
        sync_vertical_scene_to_program(&client).await?;

        let settings = self.settings.read().await.clone();
        if vertical {
            if let (Some(server), Some(key)) = (
                settings.stream_server.as_deref(),
                settings::read_stream_key(),
            ) {
                set_vertical_stream_destination(&client, server, &key).await?;
            }
        }
        let mut main_started = false;
        if horizontal {
            if !client.streaming().status().await?.active {
                if let (Some(server), Some(key)) =
                    (settings.stream_server.clone(), settings::read_stream_key())
                {
                    client
                        .raw_request(
                            "SetStreamServiceSettings",
                            json!({
                                "streamServiceType": "rtmp_custom",
                                "streamServiceSettings": {
                                    "server": server,
                                    "key": key,
                                    "use_auth": false
                                }
                            }),
                        )
                        .await
                        .context("apply the saved stream destination")?;
                }
                client
                    .streaming()
                    .start()
                    .await
                    .context("start the 16:9 stream")?;
                main_started = true;
                if !wait_for_main_stream(&client, true).await {
                    let _ = client.streaming().stop().await;
                    bail!("The 16:9 stream did not start. Check the saved stream destination in Settings.");
                }
            }
        }

        if vertical {
            let status = vertical_output_status(&client).await.unwrap_or_default();
            if !status.streaming {
                if let Err(error) = call_vertical_output(&client, "start_streaming").await {
                    if main_started {
                        let _ = client.streaming().stop().await;
                    }
                    return Err(error);
                }
                if !wait_for_vertical(&client, None, Some(true)).await {
                    let _ = call_vertical_output(&client, "stop_streaming").await;
                    if main_started {
                        let _ = client.streaming().stop().await;
                    }
                    bail!(
                        "The 9:16 stream did not start. Add a vertical stream output and destination in the Aitum Vertical dock, then try again."
                    );
                }
            }
            if let Ok(mut started) = self.vertical_stream_started.lock() {
                if started.is_none() {
                    *started = Some(Instant::now());
                }
            }
        }

        Ok(())
    }

    pub async fn stop_streaming(&self) -> Result<()> {
        let client = self.client().await?;
        let mut errors = Vec::new();
        if client.streaming().status().await?.active {
            if let Err(error) = client.streaming().stop().await {
                errors.push(format!("16:9 stream: {error:#}"));
            }
        }
        let vertical = vertical_output_status(&client).await.unwrap_or_default();
        if vertical.streaming {
            if let Err(error) = call_vertical_output(&client, "stop_streaming").await {
                errors.push(format!("9:16 stream: {error:#}"));
            } else if !wait_for_vertical(&client, None, Some(false)).await {
                errors.push("The 9:16 stream did not stop.".to_owned());
            }
        }
        if let Ok(mut started) = self.vertical_stream_started.lock() {
            *started = None;
        }
        if errors.is_empty() {
            Ok(())
        } else {
            bail!("{}", errors.join(" "));
        }
    }

    pub async fn open_projector(&self, target: String) -> Result<()> {
        let client = self.client().await?;
        match target.as_str() {
            "horizontal" => {
                client
                    .ui()
                    .open_video_mix_projector(obws::requests::ui::OpenVideoMixProjector {
                        r#type: obws::requests::ui::VideoMixType::Program,
                        location: Some(obws::requests::ui::Location::MonitorIndex(-1)),
                    })
                    .await
                    .context("open the OBS program projector")?;
            }
            "vertical" => {
                let vertical = current_vertical_scene(&client).await?;
                let scene = vertical
                    .get("scene")
                    .and_then(Value::as_str)
                    .context("Aitum did not report a current vertical scene")?;
                client
                    .ui()
                    .open_source_projector(obws::requests::ui::OpenSourceProjector {
                        source: SourceId::Name(scene),
                        location: Some(obws::requests::ui::Location::MonitorIndex(-1)),
                    })
                    .await
                    .context("open the OBS vertical scene projector")?;
            }
            _ => bail!("projector target must be horizontal or vertical"),
        }
        Ok(())
    }

    pub async fn save_preset(&self, name: String) -> Result<Vec<LayoutPreset>> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            bail!("preset name cannot be empty");
        }
        if name.len() > 64 {
            bail!("preset name is too long");
        }
        let client = self.client().await?;
        let report = self.ensure_bootstrap().await?;
        let scene_name = current_program_scene_name(&client).await?;
        let scene = report
            .scenes
            .iter()
            .find(|scene| scene.name == scene_name)
            .with_context(|| format!("save a preset from a Vidyst layout, not {scene_name}"))?;
        let camera_transform = client
            .scene_items()
            .transform(SceneId::Name(&scene.name), scene.main_camera.item_id)
            .await
            .ok();
        let screen_transform = client
            .scene_items()
            .transform(SceneId::Name(&scene.name), scene.main_screen.item_id)
            .await
            .ok();

        let radius = current_mask_radius(&client).await.unwrap_or(TILE_RADIUS) as f32;
        let camera = camera_transform.map(|transform| PresetCamera {
            x: transform.position_x / HORIZONTAL_WIDTH,
            y: transform.position_y / HORIZONTAL_HEIGHT,
            w: transform.bounds_width.max(1.0) / HORIZONTAL_WIDTH,
            h: transform.bounds_height.max(1.0) / HORIZONTAL_HEIGHT,
            radius_pct: radius,
        });
        let split_pct = if scene.id == "side_by_side" || scene.id == "side_by_side_inv" {
            screen_transform.map(|transform| {
                (transform.bounds_width.max(1.0) / HORIZONTAL_WIDTH * 100.0).clamp(35.0, 85.0)
            })
        } else {
            None
        };
        let preset = LayoutPreset {
            name: name.clone(),
            layout_id: scene.id.clone(),
            camera,
            split_pct,
            created_at_unix: now_ms() / 1000,
        };
        let mut presets = load_presets(&self.app)?;
        presets.retain(|item| item.name != name);
        presets.push(preset);
        presets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        save_presets(&self.app, &presets)?;
        Ok(presets)
    }

    pub async fn load_preset(&self, name: String) -> Result<StudioSnapshot> {
        let presets = load_presets(&self.app)?;
        let preset = presets
            .into_iter()
            .find(|preset| preset.name == name)
            .with_context(|| format!("preset {name} was not found"))?;
        let client = self.client().await?;
        let report = self.ensure_bootstrap().await?;
        let scene = report
            .scene(&preset.layout_id)
            .with_context(|| format!("layout {} is not available in OBS", preset.layout_id))?;
        client
            .scenes()
            .set_current_program_scene(SceneId::Name(&scene.name))
            .await?;
        let _ = switch_vertical_scene(&client, &scene.name).await;

        if let Some(camera) = preset.camera {
            if let Some(split_pct) = preset.split_pct {
                if preset.layout_id == "side_by_side" || preset.layout_id == "side_by_side_inv" {
                    let split = (split_pct / 100.0).clamp(0.35, 0.85);
                    let (screen_x, camera_x) = if preset.layout_id == "side_by_side" {
                        (0.02, 0.02 + split + 0.02)
                    } else {
                        (0.02 + (0.94 - split) + 0.02, 0.02)
                    };
                    set_transform(
                        &client,
                        &scene.name,
                        scene.main_screen.item_id,
                        screen_x,
                        0.03,
                        split,
                        0.94,
                        "OBS_BOUNDS_SCALE_INNER",
                        false,
                    )
                    .await?;
                    set_transform(
                        &client,
                        &scene.name,
                        scene.main_camera.item_id,
                        camera_x,
                        0.03,
                        0.94 - split,
                        0.94,
                        "OBS_BOUNDS_SCALE_OUTER",
                        true,
                    )
                    .await?;
                }
            }
            set_transform_pixels(
                &client,
                &scene.name,
                scene.main_camera.item_id,
                camera.x * HORIZONTAL_WIDTH,
                camera.y * HORIZONTAL_HEIGHT,
                camera.w * HORIZONTAL_WIDTH,
                camera.h * HORIZONTAL_HEIGHT,
                "OBS_BOUNDS_SCALE_OUTER",
                true,
            )
            .await?;
            if camera.radius_pct >= 50.0 {
                trigger_morph(&client, MORPH_TO_CIRCLE).await?;
            } else {
                trigger_morph(&client, MORPH_TO_TILE).await?;
            }
        }
        self.snapshot().await
    }

    pub async fn list_presets(&self) -> Vec<LayoutPreset> {
        load_presets(&self.app).unwrap_or_default()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn vertical_elapsed(started: &Mutex<Option<Instant>>, active: bool) -> String {
    let Ok(mut guard) = started.lock() else {
        return "00:00:00".to_owned();
    };
    if !active {
        *guard = None;
        return "00:00:00".to_owned();
    }
    let start = *guard.get_or_insert_with(Instant::now);
    let total = start.elapsed().as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

async fn vertical_output_status(client: &Client) -> Result<VerticalOutputStatus> {
    let response: obws::responses::general::VendorResponse<VerticalOutputStatus> = client
        .general()
        .call_vendor_request(obws::requests::general::CallVendorRequest {
            vendor_name: VERTICAL_VENDOR,
            request_type: "status",
            request_data: &json!({}),
        })
        .await?;
    Ok(response.response_data)
}

async fn call_vertical_output(client: &Client, request_type: &str) -> Result<()> {
    call_vertical_data(client, request_type, json!({})).await
}

async fn call_vertical_data(client: &Client, request_type: &str, data: Value) -> Result<()> {
    let response: obws::responses::general::VendorResponse<Value> = client
        .general()
        .call_vendor_request(obws::requests::general::CallVendorRequest {
            vendor_name: VERTICAL_VENDOR,
            request_type,
            request_data: &data,
        })
        .await?;
    if response
        .response_data
        .get("success")
        .and_then(Value::as_bool)
        == Some(false)
    {
        let detail = response
            .response_data
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("Aitum rejected {request_type}: {detail}");
    }
    Ok(())
}

async fn set_vertical_stream_destination(client: &Client, server: &str, key: &str) -> Result<()> {
    // Aitum keeps a separate destination per vertical output. Its vendor API
    // updates the first output when width/height are omitted, which matches the
    // single 9:16 canvas Vidyst creates.
    call_vertical_data(
        client,
        "update_stream_server",
        json!({
            "stream_server": server,
            "index": 0,
            "width": VERTICAL_WIDTH,
            "height": VERTICAL_HEIGHT,
        }),
    )
    .await
    .context("set the 9:16 stream server")?;
    call_vertical_data(
        client,
        "update_stream_key",
        json!({
            "stream_key": key,
            "index": 0,
            "width": VERTICAL_WIDTH,
            "height": VERTICAL_HEIGHT,
        }),
    )
    .await
    .context("set the 9:16 stream key")?;
    Ok(())
}

fn newest_recording(directory: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        if !matches!(
            extension.as_deref(),
            Some("mp4" | "mkv" | "mov" | "flv" | "ts" | "m4v")
        ) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) else {
            continue;
        };
        if newest
            .as_ref()
            .is_none_or(|(existing, _)| modified > *existing)
        {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, path)| path)
}

async fn wait_for_main_recording(client: &Client, active: bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if let Ok(status) = client.recording().status().await {
            if status.active == active {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    false
}

async fn wait_for_main_stream(client: &Client, active: bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(8) {
        if let Ok(status) = client.streaming().status().await {
            if status.active == active {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

async fn wait_for_vertical(
    client: &Client,
    recording: Option<bool>,
    streaming: Option<bool>,
) -> bool {
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let Ok(status) = vertical_output_status(client).await else {
            continue;
        };
        let recording_matches = recording.is_none_or(|value| status.recording == value);
        let streaming_matches = streaming.is_none_or(|value| status.streaming == value);
        if recording_matches && streaming_matches {
            return true;
        }
    }
    false
}

async fn sync_vertical_scene_to_program(client: &Client) -> Result<()> {
    let scene = current_program_scene_name(client).await?;
    if layout_id_from_scene_name(&scene).is_none() {
        return Ok(());
    }
    let response = switch_vertical_scene(client, &scene).await?;
    if response.get("success").and_then(Value::as_bool) == Some(false) {
        bail!("Aitum rejected the vertical scene sync: {response}");
    }
    Ok(())
}

async fn input_setting_string(client: &Client, input: &str, setting: &str) -> Option<String> {
    let settings = client
        .inputs()
        .settings::<Value>(InputId::Name(input))
        .await
        .ok()?;
    settings
        .settings
        .get(setting)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

async fn first_property_value(client: &Client, input: &str, property: &str) -> Option<String> {
    let items = client
        .inputs()
        .properties_list_property_items(InputId::Name(input), property)
        .await
        .ok()?;
    items
        .iter()
        .find(|item| item.enabled)
        .or_else(|| items.first())
        .map(|item| {
            item.value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| item.value.to_string())
        })
}

async fn property_display_name(
    client: &Client,
    input: &str,
    property: &str,
    value: &str,
) -> Option<String> {
    let items = client
        .inputs()
        .properties_list_property_items(InputId::Name(input), property)
        .await
        .ok()?;
    items
        .iter()
        .find(|item| item.value.as_str() == Some(value) || item.value.to_string() == value)
        .map(|item| item.name.trim_start_matches(':').trim().to_owned())
}

fn validate_audio_input(input: &str) -> Result<()> {
    match input {
        MIC_SOURCE | SCREEN_AUDIO_SOURCE => Ok(()),
        _ => bail!("unknown audio input"),
    }
}

fn monitor_type_name(monitor_type: MonitorType) -> &'static str {
    match monitor_type {
        MonitorType::None => "OBS_MONITORING_TYPE_NONE",
        MonitorType::MonitorOnly => "OBS_MONITORING_TYPE_MONITOR_ONLY",
        MonitorType::MonitorAndOutput => "OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT",
        _ => "OBS_MONITORING_TYPE_NONE",
    }
}

fn format_duration(duration: time::Duration) -> String {
    let total = duration.whole_seconds().max(0) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

fn layout_id_from_scene_name(name: &str) -> Option<String> {
    name.strip_prefix("Vidyst_").map(str::to_owned)
}

/// Re-apply the mask shape that belongs to the live vertical (or horizontal)
/// layout after bootstrap resets the mask to its rounded-tile default.
async fn sync_camera_shape_for_active_layout(client: &Client) -> Result<()> {
    let vertical = current_vertical_scene(client)
        .await
        .ok()
        .and_then(|value| {
            value
                .get("scene")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .and_then(|scene| layout_id_from_scene_name(&scene))
        .and_then(|id| builtin_layout(&id));
    let program = current_program_scene_name(client)
        .await
        .ok()
        .and_then(|scene| layout_id_from_scene_name(&scene))
        .and_then(|id| builtin_layout(&id));
    let (layout, ratio) = if let Some(layout) = vertical {
        (layout, AspectRatio::R9x16)
    } else if let Some(layout) = program {
        (layout, AspectRatio::R16x9)
    } else {
        return Ok(());
    };
    let shape = layout
        .slots_for(ratio)
        .and_then(|slots| {
            slots
                .iter()
                .find(|slot| slot.source == LayoutSource::Camera)
        })
        .map(|slot| slot.radius_pct)
        .unwrap_or(TILE_RADIUS as f32);
    if shape >= 50.0 {
        trigger_morph(client, MORPH_TO_CIRCLE).await
    } else {
        trigger_morph(client, MORPH_TO_TILE).await
    }
}

fn device_option(item: obws::responses::inputs::ListPropertyItem) -> DeviceOption {
    let value = item
        .value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| item.value.to_string());
    DeviceOption {
        name: item.name.trim_start_matches(':').trim().to_owned(),
        value,
    }
}

async fn audio_channels(client: &Client) -> Vec<AudioChannel> {
    let mut channels = Vec::new();
    for (input, label) in [
        (MIC_SOURCE, "External mic"),
        (SCREEN_AUDIO_SOURCE, "Screen audio"),
    ] {
        let muted = client
            .inputs()
            .muted(InputId::Name(input))
            .await
            .unwrap_or(false);
        let volume = client.inputs().volume(InputId::Name(input)).await.ok();
        let monitor = client
            .inputs()
            .audio_monitor_type(InputId::Name(input))
            .await
            .unwrap_or(MonitorType::None);
        channels.push(AudioChannel {
            input: input.to_owned(),
            label: label.to_owned(),
            muted,
            monitoring: monitor == MonitorType::MonitorAndOutput,
            monitor_type: monitor_type_name(monitor).to_owned(),
            volume_db: volume.as_ref().map(|value| value.db).unwrap_or(0.0),
            volume_mul: volume.as_ref().map(|value| value.mul).unwrap_or(1.0),
        });
    }
    channels
}

async fn current_program_scene_name(client: &Client) -> Result<String> {
    let response = client
        .raw_request("GetCurrentProgramScene", json!({}))
        .await
        .context("read the OBS program scene")?;
    response
        .get("currentProgramSceneName")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context("OBS did not report a program scene")
}

async fn source_visibility(client: &Client, scene: &str, screen_source: &str) -> (bool, bool) {
    let Ok(items) = client.scene_items().list(SceneId::Name(scene)).await else {
        return (false, false);
    };
    let mut camera = false;
    let mut screen = false;
    for item in items {
        if item.source_name == CAMERA_GROUP {
            camera = client
                .scene_items()
                .enabled(SceneId::Name(scene), item.id)
                .await
                .unwrap_or(false);
        } else if item.source_name == screen_source {
            screen = client
                .scene_items()
                .enabled(SceneId::Name(scene), item.id)
                .await
                .unwrap_or(false);
        }
    }
    (camera, screen)
}

async fn current_mask_radius(client: &Client) -> Option<f64> {
    let filters = client
        .filters()
        .list(SourceId::Name(CAMERA_GROUP))
        .await
        .ok()?;
    let filter = filters
        .iter()
        .find(|filter| filter.kind == "advanced_masks_filter")?;
    filter
        .settings
        .get("rectangle_corner_radius")
        .and_then(Value::as_f64)
}

async fn background_removal_enabled(client: &Client) -> bool {
    client
        .filters()
        .list(SourceId::Name(CAMERA_RAW))
        .await
        .ok()
        .and_then(|filters| {
            filters
                .into_iter()
                .find(|filter| filter.name == BACKGROUND_FILTER)
                .map(|filter| filter.enabled)
        })
        .unwrap_or(false)
}

async fn apply_recording_format(client: &Client, format: &str) -> Result<()> {
    // Advanced output profiles store the recording format in AdvOut/RecFormat2.
    // This is best-effort because older OBS profiles may use SimpleOutput instead.
    let _ = client
        .raw_request(
            "SetProfileParameter",
            json!({
                "parameterCategory": "AdvOut",
                "parameterName": "RecFormat2",
                "parameterValue": format,
            }),
        )
        .await;
    Ok(())
}

async fn input_exists(client: &Client, name: &str) -> Result<bool> {
    let inputs = client
        .inputs()
        .list(None)
        .await
        .context("list OBS inputs")?;
    Ok(inputs.iter().any(|input| input.id.name == name))
}

async fn raw_scene_items(client: &Client, scene_uuid: &str) -> Result<Vec<Value>> {
    let response = client
        .raw_request("GetSceneItemList", json!({ "sceneUuid": scene_uuid }))
        .await
        .context("list scene items")?;
    Ok(response
        .get("sceneItems")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn scene_item_id(item: &Value) -> Option<i64> {
    item.get("sceneItemId").and_then(Value::as_i64)
}

fn scene_item_by_source<'a>(items: &'a [Value], source: &str) -> Option<&'a Value> {
    items
        .iter()
        .find(|item| item.get("sourceName").and_then(Value::as_str) == Some(source))
}

async fn set_raw_item_enabled(
    client: &Client,
    scene_uuid: &str,
    item_id: i64,
    enabled: bool,
) -> Result<()> {
    client
        .raw_request(
            "SetSceneItemEnabled",
            json!({
                "sceneUuid": scene_uuid,
                "sceneItemId": item_id,
                "sceneItemEnabled": enabled,
            }),
        )
        .await
        .context("set scene item visibility")?;
    Ok(())
}

async fn ensure_raw_scene_item(client: &Client, scene_uuid: &str, source: &str) -> Result<i64> {
    let items = raw_scene_items(client, scene_uuid).await?;
    if let Some(item) = scene_item_by_source(&items, source) {
        return scene_item_id(item).context("scene item has no id");
    }
    let created = client
        .raw_request(
            "CreateSceneItem",
            json!({
                "sceneUuid": scene_uuid,
                "sourceName": source,
                "sceneItemEnabled": false,
            }),
        )
        .await
        .with_context(|| format!("add {source} to scene"))?;
    created
        .get("sceneItemId")
        .and_then(Value::as_i64)
        .with_context(|| format!("CreateSceneItem({source}) did not return sceneItemId"))
}

async fn sync_scene_window(
    client: &Client,
    scene_uuid: &str,
    screen_enabled: bool,
    show_window: bool,
) -> Result<()> {
    let items = raw_scene_items(client, scene_uuid).await?;
    let screen_item = scene_item_by_source(&items, SCREEN_SOURCE);
    let window_item = scene_item_by_source(&items, WINDOW_SOURCE);

    if show_window && screen_enabled {
        let window_id = match window_item.and_then(scene_item_id) {
            Some(id) => id,
            None => ensure_raw_scene_item(client, scene_uuid, WINDOW_SOURCE).await?,
        };
        if let Some(screen) = screen_item {
            if let Some(screen_id) = scene_item_id(screen) {
                let transform = client
                    .raw_request(
                        "GetSceneItemTransform",
                        json!({ "sceneUuid": scene_uuid, "sceneItemId": screen_id }),
                    )
                    .await
                    .context("read the screen transform")?
                    .get("sceneItemTransform")
                    .cloned()
                    .context("OBS did not return the screen transform")?;
                client
                    .raw_request(
                        "SetSceneItemTransform",
                        json!({
                            "sceneUuid": scene_uuid,
                            "sceneItemId": window_id,
                            "sceneItemTransform": transform,
                        }),
                    )
                    .await
                    .context("match the window transform to the screen slot")?;
                if let Some(index) = screen.get("sceneItemIndex").and_then(Value::as_u64) {
                    client
                        .raw_request(
                            "SetSceneItemIndex",
                            json!({
                                "sceneUuid": scene_uuid,
                                "sceneItemId": window_id,
                                "sceneItemIndex": index,
                            }),
                        )
                        .await
                        .context("place the window source in the screen slot")?;
                }
                set_raw_item_enabled(client, scene_uuid, screen_id, false).await?;
            }
        }
        set_raw_item_enabled(client, scene_uuid, window_id, true).await?;
    } else {
        if let Some(id) = screen_item.and_then(scene_item_id) {
            set_raw_item_enabled(client, scene_uuid, id, screen_enabled).await?;
        }
        if let Some(id) = window_item.and_then(scene_item_id) {
            set_raw_item_enabled(client, scene_uuid, id, false).await?;
        }
    }
    Ok(())
}

async fn sync_scene_background(
    client: &Client,
    scene_uuid: &str,
    source: &str,
    width: f32,
    height: f32,
    visible: bool,
    stretch: bool,
) -> Result<()> {
    let items = raw_scene_items(client, scene_uuid).await?;
    let existing = scene_item_by_source(&items, source);
    let item_id = match existing.and_then(scene_item_id) {
        Some(id) => id,
        None if visible => ensure_raw_scene_item(client, scene_uuid, source).await?,
        None => return Ok(()),
    };
    if visible {
        client
            .raw_request(
                "SetSceneItemTransform",
                json!({
                    "sceneUuid": scene_uuid,
                    "sceneItemId": item_id,
                    "sceneItemTransform": {
                        "positionX": 0.0,
                        "positionY": 0.0,
                        "scaleX": 1.0,
                        "scaleY": 1.0,
                        "alignment": 5,
                        "boundsType": if stretch {
                            "OBS_BOUNDS_STRETCH"
                        } else {
                            "OBS_BOUNDS_SCALE_OUTER"
                        },
                        "boundsAlignment": 0,
                        "boundsWidth": width,
                        "boundsHeight": height,
                        "cropToBounds": !stretch,
                    },
                }),
            )
            .await
            .context("fill the canvas with the background layer")?;
        client
            .raw_request(
                "SetSceneItemIndex",
                json!({
                    "sceneUuid": scene_uuid,
                    "sceneItemId": item_id,
                    "sceneItemIndex": 0,
                }),
            )
            .await
            .context("place the background behind every source")?;
    }
    set_raw_item_enabled(client, scene_uuid, item_id, visible).await?;
    Ok(())
}

fn normalize_hex_color(value: &str) -> Result<String> {
    let value = value.trim().trim_start_matches('#');
    if value.len() == 3 {
        let mut expanded = String::with_capacity(6);
        for character in value.chars() {
            expanded.push(character);
            expanded.push(character);
        }
        return normalize_hex_color(&expanded);
    }
    if value.len() != 6 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        bail!("background color must be a hex value such as #1a1b22");
    }
    Ok(format!("#{value}"))
}

fn obs_color(value: &str) -> Result<u32> {
    let normalized = normalize_hex_color(value)?;
    let channel = |start: usize| {
        u32::from_str_radix(&normalized[start..start + 2], 16)
            .context("parse background color channel")
    };
    // OBS stores source colors as AABBGGRR, not the CSS/UI order RRGGBB.
    Ok(0xFF00_0000 | (channel(5)? << 16) | (channel(3)? << 8) | channel(1)?)
}

async fn set_transform(
    client: &Client,
    scene: &str,
    item_id: i64,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bounds_type: &str,
    crop_to_bounds: bool,
) -> Result<()> {
    set_transform_pixels(
        client,
        scene,
        item_id,
        x * HORIZONTAL_WIDTH,
        y * HORIZONTAL_HEIGHT,
        w * HORIZONTAL_WIDTH,
        h * HORIZONTAL_HEIGHT,
        bounds_type,
        crop_to_bounds,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn set_transform_pixels(
    client: &Client,
    scene: &str,
    item_id: i64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    bounds_type: &str,
    crop_to_bounds: bool,
) -> Result<()> {
    client
        .raw_request(
            "SetSceneItemTransform",
            json!({
                "sceneName": scene,
                "sceneItemId": item_id,
                "sceneItemTransform": {
                    "positionX": x,
                    "positionY": y,
                    "rotation": 0.0,
                    "scaleX": 1.0,
                    "scaleY": 1.0,
                    "alignment": 5,
                    "boundsType": bounds_type,
                    "boundsAlignment": 0,
                    "boundsWidth": width,
                    "boundsHeight": height,
                    "cropToBounds": crop_to_bounds
                }
            }),
        )
        .await
        .context("apply the scene item transform")?;
    Ok(())
}

fn presets_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(settings::config_dir(app)?.join("layout-presets.json"))
}

fn load_presets(app: &AppHandle) -> Result<Vec<LayoutPreset>> {
    let path = presets_path(app)?;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(anyhow!(error).context("read layout presets")),
    };
    serde_json::from_str(&text).context("parse layout presets")
}

fn save_presets(app: &AppHandle, presets: &[LayoutPreset]) -> Result<()> {
    let path = presets_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(presets)?;
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
