//! Idempotent OBS bootstrap for the Vidyst V1 studio.
//!
//! Everything in this module is created through `obs-websocket` at runtime. The
//! code never reads or writes OBS scene-collection JSON directly.
//!
//! The four canvases supported by the layout crate are `16:9`, `9:16`, `1:1` and
//! `4:5`. V1 Phase 1 bootstraps the two canvases that exist in this OBS setup:
//! the main `1920x1080` canvas and Aitum's vertical `1080x1920` canvas.

use std::fmt::Display;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use obws::requests::filters::{
    Create as CreateFilter, SetEnabled as SetFilterEnabled, SetIndex as SetFilterIndex,
    SetSettings as SetFilterSettings,
};
use obws::requests::inputs::{Create as CreateInput, InputId, SetSettings as SetInputSettings};
use obws::requests::scenes::SceneId;
use obws::requests::sources::SaveScreenshot;
use obws::responses::filters::SourceFilter;
use obws::responses::scenes::Scene;
use obws::Client;
use serde::Serialize;
use serde_json::{json, Value};
use vidyst_layouts::{
    builtin_layouts, AspectRatio, BoundsType, Layout, ObsTransform, Slot, SourceId,
};

/// Raw camera input created inside the wrapper scene.
pub const CAMERA_RAW: &str = "Vidyst_Camera_Raw";
/// Wrapper scene that owns the camera and all camera filters.
pub const CAMERA_GROUP: &str = "Vidyst_Camera_Group";
/// Screen-capture input shared by the layout scenes.
pub const SCREEN_SOURCE: &str = "Vidyst_Screen";
/// External microphone input.
pub const MIC_SOURCE: &str = "Vidyst_Mic";
/// Desktop/screen audio input.
pub const SCREEN_AUDIO_SOURCE: &str = "Vidyst_Screen_Audio";

/// Advanced Masks rectangle filter on the camera wrapper.
pub const MASK_FILTER: &str = "Vidyst_Camera_Mask";
/// Move Value filter that morphs the camera to a circle.
pub const MORPH_TO_CIRCLE: &str = "Morph_To_Circle";
/// Move Value filter that morphs the camera back to a rounded tile.
pub const MORPH_TO_TILE: &str = "Morph_To_Tile";

/// Required scene-transition duration.
pub const TRANSITION_DURATION_MS: u64 = 350;
/// Camera mask radius that renders as a circle on a 1920x1080 source.
pub const CIRCLE_RADIUS: f64 = 540.0;
/// Default rounded-tile radius (8% of the source short side).
pub const TILE_RADIUS: f64 = 86.4;
/// DirectShow mode used when a camera has no explicit mode configured.
///
/// A dshow source created with only `video_device_id` can come up black on
/// some drivers. Supplying a concrete resolution and an automatic frame
/// interval makes OBS negotiate a working mode immediately.
pub const CAMERA_DEFAULT_RESOLUTION: &str = "1280x720";
/// Automatic frame interval for [`CAMERA_DEFAULT_RESOLUTION`].
pub const CAMERA_DEFAULT_FRAME_INTERVAL: i64 = -1;

/// Options supplied by the CLI or the Phase 2 application.
#[derive(Clone, Debug)]
pub struct BootstrapOptions {
    /// Camera device name substring or exact property value.
    pub camera_device: Option<String>,
    /// Monitor name substring or exact monitor property value.
    pub monitor: Option<String>,
    /// Microphone device name substring or exact property value.
    pub mic_device: Option<String>,
    /// Output/screen-audio device name substring or exact property value.
    pub screen_audio_device: Option<String>,
    /// Whether the camera should be activated. A disconnected camera keeps its
    /// source and scene placement but releases the physical device.
    pub camera_connected: bool,
    /// Whether the screen/window source should be activated.
    pub screen_connected: bool,
}

impl Default for BootstrapOptions {
    fn default() -> Self {
        Self {
            camera_device: None,
            monitor: None,
            mic_device: None,
            screen_audio_device: None,
            camera_connected: true,
            screen_connected: true,
        }
    }
}

/// One logged obs-websocket request. Passwords and settings bodies are never
/// stored here.
#[derive(Clone, Debug, Serialize)]
pub struct RequestLog {
    pub label: String,
    pub ok: bool,
    pub error: Option<String>,
}

impl RequestLog {
    fn ok(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ok: true,
            error: None,
        }
    }

    fn error(label: impl Into<String>, error: &impl Display) -> Self {
        Self {
            label: label.into(),
            ok: false,
            error: Some(error.to_string()),
        }
    }
}

macro_rules! obs_req {
    ($log:expr, $label:expr, $future:expr) => {
        async {
            let label = $label.to_string();
            let result = $future.await;
            match &result {
                Ok(_) => {
                    tracing::info!(request = %label, "ok");
                    $log.push(RequestLog::ok(label));
                }
                Err(error) => {
                    tracing::error!(request = %label, error = %error, "failed");
                    $log.push(RequestLog::error(label, error));
                }
            }
            result
        }
    };
}

/// Resolved source information included in the bootstrap report.
#[derive(Clone, Debug, Serialize)]
pub struct InputReport {
    pub name: String,
    pub kind: String,
    pub uuid: String,
    pub device_name: Option<String>,
    pub device_value: Option<String>,
}

/// A transform plus the layout slot that produced it.
#[derive(Clone, Debug, Serialize)]
pub struct ItemReport {
    pub source: String,
    pub item_id: i64,
    pub enabled: bool,
    pub index: i64,
    pub transform: Option<TransformReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TransformReport {
    pub position_x: f32,
    pub position_y: f32,
    pub bounds_width: f32,
    pub bounds_height: f32,
    pub bounds_type: &'static str,
    pub alignment: i32,
    pub bounds_alignment: i32,
    pub crop_to_bounds: bool,
    pub radius_pct: f32,
}

impl From<(&Slot, ObsTransform)> for TransformReport {
    fn from((slot, transform): (&Slot, ObsTransform)) -> Self {
        Self {
            position_x: transform.position_x,
            position_y: transform.position_y,
            bounds_width: transform.bounds_width,
            bounds_height: transform.bounds_height,
            bounds_type: match transform.bounds_type {
                vidyst_layouts::BoundsType::ScaleToInnerBounds => "OBS_BOUNDS_SCALE_INNER",
                vidyst_layouts::BoundsType::ScaleToOuterBounds => "OBS_BOUNDS_SCALE_OUTER",
            },
            alignment: transform.alignment,
            bounds_alignment: transform.bounds_alignment,
            crop_to_bounds: transform.crop_to_bounds,
            radius_pct: slot.radius_pct,
        }
    }
}

/// Per-layout scene data for both canvases.
#[derive(Clone, Debug, Serialize)]
pub struct SceneReport {
    pub id: String,
    pub name: String,
    pub main_scene_uuid: String,
    pub vertical_scene_uuid: String,
    pub main_screen: ItemReport,
    pub main_camera: ItemReport,
    pub vertical_screen: ItemReport,
    pub vertical_camera: ItemReport,
}

#[derive(Clone, Debug, Serialize)]
pub struct FilterReport {
    pub source: String,
    pub mask_filter: String,
    pub mask_kind: String,
    pub base_radius: f64,
    pub circle_filter: String,
    pub circle_target_radius: f64,
    pub tile_filter: String,
    pub tile_target_radius: f64,
    pub duration_ms: u64,
    pub enabled_after_bootstrap: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TransitionReport {
    pub name: String,
    pub duration_ms: u64,
}

/// In-memory result of one bootstrap run. This is the source of truth for IDs
/// used by the validation tests and, later, the Tauri application.
#[derive(Clone, Debug, Serialize)]
pub struct BootstrapReport {
    pub generated_at_unix: u64,
    pub main_canvas_uuid: String,
    pub vertical_canvas_uuid: String,
    pub camera_group_uuid: String,
    pub camera: InputReport,
    pub screen: InputReport,
    pub mic: InputReport,
    pub screen_audio: InputReport,
    pub filters: FilterReport,
    pub transition: TransitionReport,
    pub scenes: Vec<SceneReport>,
    pub requests: Vec<RequestLog>,
}

impl BootstrapReport {
    pub fn scene(&self, id: &str) -> Option<&SceneReport> {
        self.scenes.iter().find(|scene| scene.id == id)
    }

    pub fn scene_name(id: &str) -> String {
        format!("Vidyst_{id}")
    }
}

/// Create or reconcile every object required by the Phase 1 spike.
pub async fn run_bootstrap(client: &Client, options: &BootstrapOptions) -> Result<BootstrapReport> {
    let mut requests = Vec::new();
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    let canvases = obs_req!(requests, "GetCanvasList", client.canvases().list()).await?;
    let main_canvas = canvases
        .iter()
        .find(|canvas| {
            canvas
                .flags
                .contains(obws::responses::canvases::CanvasFlags::MAIN)
        })
        .context("OBS did not report a main canvas")?;
    let vertical_canvas = canvases
        .iter()
        .find(|canvas| canvas.id.name.contains("Vertical"))
        .context("Aitum Vertical canvas was not found")?;

    let transitions = obs_req!(
        requests,
        "GetSceneTransitionList",
        client.transitions().list()
    )
    .await?;
    if !transitions
        .transitions
        .iter()
        .any(|transition| transition.id.name == "Move")
    {
        bail!(
            "the Move transition is not present in OBS; add it in the Scene Transitions dock before running the spike"
        );
    }

    let camera_group_uuid = ensure_main_scene(client, &mut requests, CAMERA_GROUP).await?;

    let camera = ensure_camera_input(
        client,
        &mut requests,
        options.camera_device.as_deref(),
        options.camera_connected,
    )
    .await?;
    let screen = ensure_device_input(
        client,
        &mut requests,
        &camera_group_uuid,
        SCREEN_SOURCE,
        "monitor_capture",
        "monitor_id",
        options.monitor.as_deref(),
        "monitor",
        false,
        options.screen_connected,
    )
    .await?;
    let mic = ensure_audio_input(
        client,
        &mut requests,
        &camera_group_uuid,
        MIC_SOURCE,
        "wasapi_input_capture",
        options.mic_device.as_deref(),
        "microphone",
    )
    .await?;
    let screen_audio = ensure_audio_input(
        client,
        &mut requests,
        &camera_group_uuid,
        SCREEN_AUDIO_SOURCE,
        "wasapi_output_capture",
        options.screen_audio_device.as_deref(),
        "screen audio",
    )
    .await?;

    let camera_uuid = camera.uuid.clone();
    let camera_item_id = ensure_group_item(
        client,
        &mut requests,
        &camera_group_uuid,
        &camera_uuid,
        true,
    )
    .await?;
    let camera_fill = ObsTransform {
        position_x: 0.0,
        position_y: 0.0,
        bounds_width: 1920.0,
        bounds_height: 1080.0,
        alignment: 5,
        bounds_alignment: 0,
        bounds_type: BoundsType::ScaleToOuterBounds,
        crop_to_bounds: true,
    };
    obs_req!(
        requests,
        "SetSceneItemTransform(Vidyst_Camera_Raw fill group)",
        client.raw_request(
            "SetSceneItemTransform",
            json!({
                "sceneUuid": camera_group_uuid,
                "sceneItemId": camera_item_id,
                "sceneItemTransform": transform_json(&camera_fill),
            })
        )
    )
    .await?;

    let filters = ensure_camera_filters(client, &mut requests).await?;

    obs_req!(
        requests,
        "SetCurrentSceneTransition(Move)",
        client.transitions().set_current("Move")
    )
    .await?;
    obs_req!(
        requests,
        "SetCurrentSceneTransitionDuration(350)",
        client
            .transitions()
            .set_current_duration(time::Duration::milliseconds(TRANSITION_DURATION_MS as i64))
    )
    .await?;

    let mut scenes = Vec::new();
    for layout in builtin_layouts() {
        scenes.push(
            ensure_layout_scene(
                client,
                &mut requests,
                &layout,
                &screen,
                &camera_group_uuid,
                &vertical_canvas.id.uuid.to_string(),
            )
            .await?,
        );
    }

    let transition = obs_req!(
        requests,
        "GetCurrentSceneTransition",
        client.transitions().current()
    )
    .await?;

    Ok(BootstrapReport {
        generated_at_unix,
        main_canvas_uuid: main_canvas.id.uuid.to_string(),
        vertical_canvas_uuid: vertical_canvas.id.uuid.to_string(),
        camera_group_uuid,
        camera,
        screen,
        mic,
        screen_audio,
        filters,
        transition: TransitionReport {
            name: transition.id.name,
            duration_ms: transition
                .duration
                .map(|duration| {
                    u64::try_from(duration.whole_milliseconds()).unwrap_or(TRANSITION_DURATION_MS)
                })
                .unwrap_or(TRANSITION_DURATION_MS),
        },
        scenes,
        requests,
    })
}

async fn ensure_main_scene(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    name: &str,
) -> Result<String> {
    let scenes = obs_req!(
        requests,
        format!("GetSceneList({name})"),
        client.scenes().list()
    )
    .await?;
    if let Some(scene) = scenes.scenes.iter().find(|scene| scene.id.name == name) {
        return Ok(scene.id.uuid.to_string());
    }

    let uuid = obs_req!(
        requests,
        format!("CreateScene({name})"),
        client.scenes().create(name)
    )
    .await
    .with_context(|| format!("create main scene {name}"))?;
    Ok(uuid.to_string())
}

async fn ensure_camera_input(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    wanted: Option<&str>,
    connected: bool,
) -> Result<InputReport> {
    let inputs = obs_req!(requests, "GetInputList", client.inputs().list(None)).await?;
    let existing = inputs.iter().find(|input| input.id.name == CAMERA_RAW);

    let (kind, uuid, current_settings) = if let Some(input) = existing {
        let settings = obs_req!(
            requests,
            "GetInputSettings(Vidyst_Camera_Raw)",
            client.inputs().settings::<Value>(InputId::Name(CAMERA_RAW))
        )
        .await?;
        (settings.kind, input.id.uuid.to_string(), settings.settings)
    } else {
        // When the persisted state is disconnected, create the source with an
        // empty device. DirectShow still marks the source active, so a later
        // SetInputSettings(video_device_id) starts capture without recreating
        // the input, but no camera is held while disconnected.
        let initial_settings = if connected {
            camera_settings(None)
        } else {
            camera_settings_disabled()
        };
        obs_req!(
            requests,
            "CreateInput(Vidyst_Camera_Raw)",
            client.inputs().create(CreateInput {
                scene: SceneId::Name(CAMERA_GROUP),
                input: CAMERA_RAW,
                kind: "dshow_input",
                settings: Some(initial_settings.clone()),
                enabled: Some(true),
            })
        )
        .await?;

        // `CreateInput` always returns a UUID. Re-list so the report comes
        // from OBS, not from assumptions about the response shape.
        let inputs = obs_req!(
            requests,
            "GetInputList(after camera create)",
            client.inputs().list(None)
        )
        .await?;
        let input = inputs
            .iter()
            .find(|input| input.id.name == CAMERA_RAW)
            .context("camera input was created but is missing from GetInputList")?;
        (
            "dshow_input".to_owned(),
            input.id.uuid.to_string(),
            initial_settings,
        )
    };

    if !connected {
        obs_req!(
            requests,
            "SetInputSettings(Vidyst_Camera_Raw disconnected)",
            client.inputs().set_settings(SetInputSettings {
                input: InputId::Name(CAMERA_RAW),
                settings: &camera_settings_disabled(),
                overlay: Some(true),
            })
        )
        .await?;
        return Ok(InputReport {
            name: CAMERA_RAW.to_owned(),
            kind,
            uuid,
            device_name: None,
            device_value: None,
        });
    }

    let current_device = current_settings
        .get("video_device_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    // Keep a working saved device. If it disappeared, or the caller asked for
    // a specific camera, fall back to OBS's first enabled device.
    let selection = select_property_item(
        client,
        requests,
        CAMERA_RAW,
        "video_device_id",
        wanted.or(current_device.as_deref()),
        "camera",
    )
    .await?;
    let selected_value = selection
        .as_ref()
        .and_then(|item| item.value.as_str())
        .map(str::to_owned);
    let device_changed = wanted.is_some()
        || current_device.is_none()
        || selected_value.as_deref() != current_device.as_deref();
    let has_mode = current_settings.get("resolution").is_some()
        && current_settings.get("frame_interval").is_some();
    if device_changed || !has_mode {
        let desired = camera_settings(selected_value.as_deref());
        obs_req!(
            requests,
            "SetInputSettings(Vidyst_Camera_Raw)",
            client.inputs().set_settings(SetInputSettings {
                input: InputId::Name(CAMERA_RAW),
                settings: &desired,
                overlay: Some(true),
            })
        )
        .await?;
    }

    Ok(InputReport {
        name: CAMERA_RAW.to_owned(),
        kind,
        uuid,
        device_name: selection
            .as_ref()
            .map(|item| property_item_display_name(&item.name)),
        device_value: selection.map(|item| property_value_string(item.value)),
    })
}

fn camera_settings(device: Option<&str>) -> Value {
    let mut settings = json!({
        "resolution": CAMERA_DEFAULT_RESOLUTION,
        "frame_interval": CAMERA_DEFAULT_FRAME_INTERVAL,
        "video_format": 0,
    });
    if let Some(device) = device {
        settings["video_device_id"] = Value::String(device.to_owned());
    }
    settings
}

fn camera_settings_disabled() -> Value {
    let mut settings = camera_settings(None);
    settings["video_device_id"] = Value::String(String::new());
    settings
}

/// Ensure a monitor or audio input exists. New visual inputs are created in the
/// wrapper scene and immediately removed again so only the global input remains.
async fn ensure_device_input(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    group_uuid: &str,
    name: &str,
    kind: &str,
    property: &str,
    wanted: Option<&str>,
    description: &str,
    item_enabled: bool,
    connected: bool,
) -> Result<InputReport> {
    let inputs = obs_req!(
        requests,
        format!("GetInputList({name})"),
        client.inputs().list(None)
    )
    .await?;
    let existed = inputs.iter().any(|input| input.id.name == name);

    if !existed {
        obs_req!(
            requests,
            format!("CreateInput({name})"),
            client.inputs().create(CreateInput {
                scene: SceneId::Name(CAMERA_GROUP),
                input: name,
                kind,
                settings: Some(json!({})),
                enabled: Some(item_enabled),
            })
        )
        .await?;
    }

    let current_settings = if existed {
        obs_req!(
            requests,
            format!("GetInputSettings({name})"),
            client.inputs().settings::<Value>(InputId::Name(name))
        )
        .await?
        .settings
    } else {
        json!({})
    };
    let current_value = current_settings
        .get(property)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());

    let inputs = obs_req!(
        requests,
        format!("GetInputList(after create {name})"),
        client.inputs().list(None)
    )
    .await?;
    let input = inputs
        .iter()
        .find(|input| input.id.name == name)
        .with_context(|| format!("{description} input {name} is missing from GetInputList"))?;
    ensure_group_item(
        client,
        requests,
        group_uuid,
        &input.id.uuid.to_string(),
        item_enabled,
    )
    .await?;

    let selection =
        select_property_item(client, requests, name, property, wanted, description).await?;
    let selected_value = selection
        .as_ref()
        .and_then(|item| item.value.as_str())
        .map(str::to_owned);
    let should_apply = !existed
        || wanted.is_some()
        || !connected
        || current_value.is_none()
        || selected_value.as_deref() != current_value;
    if should_apply {
        let value = if connected {
            selected_value
                .as_deref()
                .map(|value| Value::String(value.to_owned()))
                .unwrap_or_else(|| Value::String(String::new()))
        } else {
            Value::String(String::new())
        };
        let settings = json!({ property: value });
        obs_req!(
            requests,
            format!("SetInputSettings({name})"),
            client.inputs().set_settings(SetInputSettings {
                input: InputId::Name(name),
                settings: &settings,
                overlay: Some(true),
            })
        )
        .await?;
    }

    let inputs = obs_req!(
        requests,
        format!("GetInputList(after {name})"),
        client.inputs().list(None)
    )
    .await?;
    let input = inputs
        .iter()
        .find(|input| input.id.name == name)
        .with_context(|| format!("{description} input {name} is missing from GetInputList"))?;
    // A released device must not overwrite the user's saved selection with
    // whichever device OBS happened to list first.
    let reported = if connected { selection } else { None };

    Ok(InputReport {
        name: name.to_owned(),
        kind: input.kind.clone(),
        uuid: input.id.uuid.to_string(),
        device_name: reported
            .as_ref()
            .map(|item| property_item_display_name(&item.name)),
        device_value: reported.map(|item| property_value_string(item.value)),
    })
}

async fn ensure_audio_input(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    group_uuid: &str,
    name: &str,
    kind: &str,
    wanted: Option<&str>,
    description: &str,
) -> Result<InputReport> {
    ensure_device_input(
        client,
        requests,
        group_uuid,
        name,
        kind,
        "device_id",
        wanted,
        description,
        true,
        true,
    )
    .await
}

#[derive(Clone, Debug)]
struct PropertyItem {
    name: String,
    value: Value,
}

fn property_value_string(value: Value) -> String {
    match value {
        Value::String(value) => value,
        other => other.to_string(),
    }
}

/// Monitor property items are labelled like `": 1920x1080 @ 0,0"` by OBS.
/// Strip the leading separator for human-readable reports.
fn property_item_display_name(name: &str) -> String {
    name.trim_start_matches(':').trim().to_owned()
}

async fn select_property_item(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    input: &str,
    property: &str,
    wanted: Option<&str>,
    description: &str,
) -> Result<Option<PropertyItem>> {
    let items = obs_req!(
        requests,
        format!("GetInputPropertiesListPropertyItems({input}.{property})"),
        client
            .inputs()
            .properties_list_property_items(InputId::Name(input), property)
    )
    .await?;

    let selected = wanted
        .and_then(|wanted| {
            items.iter().find(|item| {
                item.name.contains(wanted)
                    || item.value.as_str().is_some_and(|value| value == wanted)
            })
        })
        .or_else(|| items.iter().find(|item| item.enabled))
        .or_else(|| items.first());

    let Some(item) = selected else {
        bail!("OBS returned no {description} devices for {input}");
    };

    Ok(Some(PropertyItem {
        name: item.name.clone(),
        value: item.value.clone(),
    }))
}

async fn ensure_group_item(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    group_uuid: &str,
    source_uuid: &str,
    enabled: bool,
) -> Result<i64> {
    let items = list_scene_items(client, requests, group_uuid).await?;
    let mut matching: Vec<_> = items
        .iter()
        .filter(|item| item.source_uuid == source_uuid)
        .map(|item| item.id)
        .collect();
    matching.sort_unstable();

    let item_id = if let Some(item_id) = matching.first().copied() {
        for duplicate in matching.iter().skip(1) {
            obs_req!(
                requests,
                format!("RemoveSceneItem(Group duplicate {duplicate})"),
                client.raw_request(
                    "RemoveSceneItem",
                    json!({ "sceneUuid": group_uuid, "sceneItemId": duplicate })
                )
            )
            .await?;
        }
        item_id
    } else {
        let item = obs_req!(
            requests,
            "CreateSceneItem(CameraGroup)",
            client.raw_request(
                "CreateSceneItem",
                json!({
                    "sceneUuid": group_uuid,
                    "sourceUuid": source_uuid,
                    "sceneItemEnabled": enabled,
                })
            )
        )
        .await?;
        item.get("sceneItemId")
            .and_then(Value::as_i64)
            .context("CreateSceneItem(CameraGroup) did not return sceneItemId")?
    };
    set_item_enabled(client, requests, group_uuid, item_id, enabled).await?;
    Ok(item_id)
}

async fn ensure_camera_filters(
    client: &Client,
    requests: &mut Vec<RequestLog>,
) -> Result<FilterReport> {
    let current = obs_req!(
        requests,
        "GetSourceFilterList(Vidyst_Camera_Group)",
        client
            .filters()
            .list(obws::requests::sources::SourceId::Name(CAMERA_GROUP))
    )
    .await?;

    let mask_settings = mask_settings(TILE_RADIUS);
    if !current.iter().any(|filter| filter.name == MASK_FILTER) {
        obs_req!(
            requests,
            "CreateSourceFilter(Vidyst_Camera_Mask)",
            client.filters().create(CreateFilter {
                source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
                filter: MASK_FILTER,
                kind: "advanced_masks_filter",
                settings: Some(&mask_settings),
            })
        )
        .await?;
    }

    // Apply the mask settings after the filter exists. Advanced Masks clamps
    // settings against source dimensions during its first update, so sending
    // the radius only as part of CreateSourceFilter can leave it at zero.
    obs_req!(
        requests,
        "SetSourceFilterSettings(Vidyst_Camera_Mask)",
        client.filters().set_settings(SetFilterSettings {
            source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
            filter: MASK_FILTER,
            settings: &mask_settings,
            overlay: Some(true),
        })
    )
    .await?;

    ensure_morph_filter(client, requests, MORPH_TO_CIRCLE, CIRCLE_RADIUS, &current).await?;
    ensure_morph_filter(client, requests, MORPH_TO_TILE, TILE_RADIUS, &current).await?;

    // Keep the mask at the bottom of the filter chain and both morphers
    // disabled after bootstrap. The switcher enables exactly one morpher when a
    // layout changes.
    set_filter_index(client, requests, MASK_FILTER, 0).await?;
    set_filter_enabled(client, requests, MORPH_TO_CIRCLE, false).await?;
    set_filter_enabled(client, requests, MORPH_TO_TILE, false).await?;
    set_filter_enabled(client, requests, MASK_FILTER, true).await?;

    let filters = obs_req!(
        requests,
        "GetSourceFilterList(after setup)",
        client
            .filters()
            .list(obws::requests::sources::SourceId::Name(CAMERA_GROUP))
    )
    .await?;
    let mask = filters
        .iter()
        .find(|filter| filter.name == MASK_FILTER)
        .context("camera mask filter is missing after setup")?;
    let mask_kind = mask.kind.clone();
    let base_radius = mask_radius(mask).unwrap_or(TILE_RADIUS);
    let enabled = filters
        .iter()
        .filter(|filter| filter.name == MASK_FILTER)
        .all(|filter| filter.enabled);
    let morphs_enabled = filters
        .iter()
        .filter(|filter| filter.name == MORPH_TO_CIRCLE || filter.name == MORPH_TO_TILE)
        .any(|filter| filter.enabled);

    Ok(FilterReport {
        source: CAMERA_GROUP.to_owned(),
        mask_filter: MASK_FILTER.to_owned(),
        mask_kind,
        base_radius,
        circle_filter: MORPH_TO_CIRCLE.to_owned(),
        circle_target_radius: CIRCLE_RADIUS,
        tile_filter: MORPH_TO_TILE.to_owned(),
        tile_target_radius: TILE_RADIUS,
        duration_ms: TRANSITION_DURATION_MS,
        enabled_after_bootstrap: enabled && !morphs_enabled,
    })
}

async fn ensure_morph_filter(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    name: &str,
    target_radius: f64,
    current: &[SourceFilter],
) -> Result<()> {
    let settings = move_value_settings(target_radius);
    if current.iter().any(|filter| filter.name == name) {
        obs_req!(
            requests,
            format!("SetSourceFilterSettings({name})"),
            client.filters().set_settings(SetFilterSettings {
                source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
                filter: name,
                settings: &settings,
                overlay: Some(true),
            })
        )
        .await?;
    } else {
        obs_req!(
            requests,
            format!("CreateSourceFilter({name})"),
            client.filters().create(CreateFilter {
                source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
                filter: name,
                kind: "move_value_filter",
                settings: Some(&settings),
            })
        )
        .await?;
    }
    Ok(())
}

fn mask_settings(radius: f64) -> Value {
    json!({
        "mask_type": 1,
        "mask_effect": 1,
        "shape_type": 1,
        "shape_relative": false,
        "shape_frame_check": false,
        "shape_invert": false,
        "shape_center_x": 960.0,
        "shape_center_y": 540.0,
        "shape_rotation": 0.0,
        "position_x": 0.0,
        "position_y": 0.0,
        "position_scale": 100.0,
        "source_zoom": 100.0,
        "scale_type": 1,
        "rectangle_width": 1920.0,
        "rectangle_height": 1080.0,
        "rectangle_corner_type": 1,
        "rectangle_corner_radius": radius,
        "rectangle_corner_radius_tl": radius,
        "rectangle_corner_radius_tr": radius,
        "rectangle_corner_radius_bl": radius,
        "rectangle_corner_radius_br": radius,
        "shape_corner_radius": 0.0,
        "mask_gradient_debug": false,
    })
}

fn move_value_settings(target_radius: f64) -> Value {
    json!({
        "filter": MASK_FILTER,
        "single_setting": true,
        "move_value_type": 0,
        "setting_name": "rectangle_corner_radius",
        "value_type": 2,
        "setting_float": target_radius,
        "setting_float_min": 0.0,
        "setting_float_max": 540.0,
        "custom_duration": true,
        "duration": TRANSITION_DURATION_MS,
        "start_delay": 0,
        "end_delay": 0,
        "easing_match": 3,
        "easing_function_match": 2,
        "enabled_match_moving": true,
        "start_trigger": 5,
        "stop_trigger": 0,
        "start_trigger_udp_port": 3000,
        "stop_trigger_udp_port": 3000,
    })
}

async fn ensure_layout_scene(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    layout: &Layout,
    screen: &InputReport,
    camera_group_uuid: &str,
    vertical_canvas_uuid: &str,
) -> Result<SceneReport> {
    let name = BootstrapReport::scene_name(&layout.id);
    let main_scene_uuid = ensure_main_scene(client, requests, &name).await?;
    let vertical_scene_uuid =
        ensure_vertical_scene(client, requests, &name, vertical_canvas_uuid).await?;

    let main_screen = configure_item(
        client,
        requests,
        &main_scene_uuid,
        &screen.uuid,
        SCREEN_SOURCE,
        layout.slots_for(AspectRatio::R16x9),
        AspectRatio::R16x9,
    )
    .await?;
    let main_camera = configure_item(
        client,
        requests,
        &main_scene_uuid,
        camera_group_uuid,
        CAMERA_GROUP,
        layout.slots_for(AspectRatio::R16x9),
        AspectRatio::R16x9,
    )
    .await?;
    let vertical_screen = configure_item(
        client,
        requests,
        &vertical_scene_uuid,
        &screen.uuid,
        SCREEN_SOURCE,
        layout.slots_for(AspectRatio::R9x16),
        AspectRatio::R9x16,
    )
    .await?;
    let vertical_camera = configure_item(
        client,
        requests,
        &vertical_scene_uuid,
        camera_group_uuid,
        CAMERA_GROUP,
        layout.slots_for(AspectRatio::R9x16),
        AspectRatio::R9x16,
    )
    .await?;

    Ok(SceneReport {
        id: layout.id.clone(),
        name,
        main_scene_uuid,
        vertical_scene_uuid,
        main_screen,
        main_camera,
        vertical_screen,
        vertical_camera,
    })
}

#[derive(Clone, Debug)]
struct CanvasScene {
    name: String,
    uuid: String,
}

async fn list_canvas_scenes(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    canvas_uuid: &str,
) -> Result<Vec<CanvasScene>> {
    let response = obs_req!(
        requests,
        format!("GetSceneList(canvas {canvas_uuid})"),
        client.raw_request("GetSceneList", json!({ "canvasUuid": canvas_uuid }))
    )
    .await?;
    let scenes = response
        .get("scenes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(scenes
        .iter()
        .filter_map(|scene| {
            Some(CanvasScene {
                name: scene.get("sceneName")?.as_str()?.to_owned(),
                uuid: scene.get("sceneUuid")?.as_str()?.to_owned(),
            })
        })
        .collect())
}

async fn ensure_vertical_scene(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    name: &str,
    canvas_uuid: &str,
) -> Result<String> {
    let known = list_canvas_scenes(client, requests, canvas_uuid).await?;
    if let Some(scene) = known.iter().find(|scene| scene.name == name) {
        return Ok(scene.uuid.clone());
    }

    let created = obs_req!(
        requests,
        format!("CreateScene({name}, vertical)"),
        client.raw_request(
            "CreateScene",
            json!({ "sceneName": name, "canvasUuid": canvas_uuid })
        )
    )
    .await?;
    created
        .get("sceneUuid")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("CreateScene({name}) did not return sceneUuid"))
}

async fn configure_item(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    scene_uuid: &str,
    source_uuid: &str,
    source_name: &str,
    slots: Option<&[Slot]>,
    ratio: AspectRatio,
) -> Result<ItemReport> {
    let slot = slots.and_then(|slots| {
        let source = if source_name == CAMERA_RAW || source_name == CAMERA_GROUP {
            SourceId::Camera
        } else {
            SourceId::Screen
        };
        slots.iter().find(|slot| slot.source == source)
    });

    let items = list_scene_items(client, requests, scene_uuid).await?;
    let mut matching: Vec<_> = items
        .iter()
        .filter(|item| item.source_uuid == source_uuid)
        .map(|item| item.id)
        .collect();
    matching.sort_unstable();

    let item_id = if let Some(item_id) = matching.first().copied() {
        for duplicate in matching.iter().skip(1) {
            obs_req!(
                requests,
                format!("RemoveSceneItem({source_name}, duplicate {duplicate})"),
                client.raw_request(
                    "RemoveSceneItem",
                    json!({ "sceneUuid": scene_uuid, "sceneItemId": duplicate })
                )
            )
            .await?;
        }
        item_id
    } else {
        let created = obs_req!(
            requests,
            format!("CreateSceneItem({source_name})"),
            client.raw_request(
                "CreateSceneItem",
                json!({
                    "sceneUuid": scene_uuid,
                    "sourceUuid": source_uuid,
                    "sceneItemEnabled": false,
                })
            )
        )
        .await?;
        created
            .get("sceneItemId")
            .and_then(Value::as_i64)
            .with_context(|| format!("CreateSceneItem({source_name}) did not return sceneItemId"))?
    };

    let transform = if let Some(slot) = slot {
        let (canvas_w, canvas_h) = ratio.canvas_size();
        let transform = slot.obs_transform(canvas_w, canvas_h);
        let value = transform_json(&transform);
        obs_req!(
            requests,
            format!("SetSceneItemTransform({source_name})"),
            client.raw_request(
                "SetSceneItemTransform",
                json!({
                    "sceneUuid": scene_uuid,
                    "sceneItemId": item_id,
                    "sceneItemTransform": value,
                })
            )
        )
        .await?;
        Some(TransformReport::from((slot, transform)))
    } else {
        None
    };

    // Source z-order: screen at 0, camera above it at 1.
    let index = if source_name == CAMERA_RAW || source_name == CAMERA_GROUP {
        1
    } else {
        0
    };
    obs_req!(
        requests,
        format!("SetSceneItemIndex({source_name}, {index})"),
        client.raw_request(
            "SetSceneItemIndex",
            json!({
                "sceneUuid": scene_uuid,
                "sceneItemId": item_id,
                "sceneItemIndex": index,
            })
        )
    )
    .await?;

    let enabled = slot.is_some();
    set_item_enabled(client, requests, scene_uuid, item_id, enabled).await?;

    Ok(ItemReport {
        source: source_name.to_owned(),
        item_id,
        enabled,
        index,
        transform,
    })
}

fn transform_json(transform: &ObsTransform) -> Value {
    json!({
        "positionX": transform.position_x,
        "positionY": transform.position_y,
        "boundsType": match transform.bounds_type {
            vidyst_layouts::BoundsType::ScaleToInnerBounds => "OBS_BOUNDS_SCALE_INNER",
            vidyst_layouts::BoundsType::ScaleToOuterBounds => "OBS_BOUNDS_SCALE_OUTER",
        },
        "boundsAlignment": transform.bounds_alignment,
        "boundsWidth": transform.bounds_width,
        "boundsHeight": transform.bounds_height,
        "cropToBounds": transform.crop_to_bounds,
        "alignment": transform.alignment,
        "scaleX": 1.0,
        "scaleY": 1.0,
        "rotation": 0.0,
        "cropLeft": 0,
        "cropRight": 0,
        "cropTop": 0,
        "cropBottom": 0,
    })
}

async fn list_scene_items(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    scene_uuid: &str,
) -> Result<Vec<RawSceneItem>> {
    let response = obs_req!(
        requests,
        format!("GetSceneItemList({scene_uuid})"),
        client.raw_request("GetSceneItemList", json!({ "sceneUuid": scene_uuid }))
    )
    .await?;
    let items = response
        .get("sceneItems")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(items
        .iter()
        .filter_map(|item| {
            Some(RawSceneItem {
                id: item.get("sceneItemId")?.as_i64()?,
                source_uuid: item.get("sourceUuid")?.as_str()?.to_owned(),
            })
        })
        .collect())
}

#[derive(Clone, Debug)]
struct RawSceneItem {
    id: i64,
    source_uuid: String,
}

async fn set_item_enabled(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    scene_uuid: &str,
    item_id: i64,
    enabled: bool,
) -> Result<()> {
    obs_req!(
        requests,
        format!("SetSceneItemEnabled({item_id}, {enabled})"),
        client.raw_request(
            "SetSceneItemEnabled",
            json!({
                "sceneUuid": scene_uuid,
                "sceneItemId": item_id,
                "sceneItemEnabled": enabled,
            })
        )
    )
    .await?;
    Ok(())
}

async fn set_filter_enabled(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    filter: &str,
    enabled: bool,
) -> Result<()> {
    obs_req!(
        requests,
        format!("SetSourceFilterEnabled({filter}, {enabled})"),
        client.filters().set_enabled(SetFilterEnabled {
            source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
            filter,
            enabled,
        })
    )
    .await?;
    Ok(())
}

async fn set_filter_index(
    client: &Client,
    requests: &mut Vec<RequestLog>,
    filter: &str,
    index: u32,
) -> Result<()> {
    obs_req!(
        requests,
        format!("SetSourceFilterIndex({filter}, {index})"),
        client.filters().set_index(SetFilterIndex {
            source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
            filter,
            index,
        })
    )
    .await?;
    Ok(())
}

/// Read the current Advanced Masks corner radius from OBS.
pub async fn current_mask_radius(client: &Client) -> Result<f64> {
    let filters = client
        .filters()
        .list(obws::requests::sources::SourceId::Name(CAMERA_GROUP))
        .await?;
    let mask = filters
        .iter()
        .find(|filter| filter.name == MASK_FILTER)
        .context("Vidyst_Camera_Mask is missing")?;
    mask_radius(mask).context("Vidyst_Camera_Mask has no rectangle_corner_radius")
}

fn mask_radius(mask: &SourceFilter) -> Option<f64> {
    mask.settings
        .get("rectangle_corner_radius")
        .and_then(Value::as_f64)
}

/// Enable one morph filter and disable the other. The Move Value filters are
/// edge-triggered on enable, so a disabled->enabled transition is required.
pub async fn trigger_morph(client: &Client, filter: &str) -> Result<()> {
    let other = match filter {
        MORPH_TO_CIRCLE => MORPH_TO_TILE,
        MORPH_TO_TILE => MORPH_TO_CIRCLE,
        _ => bail!("unknown morph filter {filter}"),
    };
    // A large corner radius only becomes a true circle when the mask rectangle
    // is square. Keep the mask centered on the 1920x1080 camera source, and
    // restore the full-width rectangle for rounded tiles.
    let (width, height) = mask_dimensions(if filter == MORPH_TO_CIRCLE {
        CIRCLE_RADIUS
    } else {
        TILE_RADIUS
    });
    client
        .filters()
        .set_settings(SetFilterSettings {
            source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
            filter: MASK_FILTER,
            settings: &json!({
                "shape_center_x": 960.0,
                "shape_center_y": 540.0,
                "rectangle_width": width,
                "rectangle_height": height,
            }),
            overlay: Some(true),
        })
        .await?;
    // Move Value filters are edge-triggered on enable. Disable both first so
    // repeated switches to the same shape restart from the current radius
    // instead of leaving the previous filter enabled and stale.
    for name in [filter, other] {
        client
            .filters()
            .set_enabled(SetFilterEnabled {
                source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
                filter: name,
                enabled: false,
            })
            .await?;
    }
    client
        .filters()
        .set_enabled(SetFilterEnabled {
            source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
            filter,
            enabled: true,
        })
        .await?;
    Ok(())
}

/// Disable both Move Value filters.
pub async fn reset_morph_filters(client: &Client) -> Result<()> {
    for filter in [MORPH_TO_CIRCLE, MORPH_TO_TILE] {
        client
            .filters()
            .set_enabled(SetFilterEnabled {
                source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
                filter,
                enabled: false,
            })
            .await?;
    }
    Ok(())
}

/// Set the Advanced Masks corner radius directly. Used by tests to put the
/// camera into a known start state.
pub async fn set_mask_radius(client: &Client, radius: f64) -> Result<()> {
    let (width, height) = mask_dimensions(radius);
    client
        .filters()
        .set_settings(SetFilterSettings {
            source: obws::requests::sources::SourceId::Name(CAMERA_GROUP),
            filter: MASK_FILTER,
            settings: &json!({
                "shape_center_x": 960.0,
                "shape_center_y": 540.0,
                "rectangle_width": width,
                "rectangle_height": height,
                "rectangle_corner_type": 1,
                "rectangle_corner_radius": radius,
                "rectangle_corner_radius_tl": radius,
                "rectangle_corner_radius_tr": radius,
                "rectangle_corner_radius_bl": radius,
                "rectangle_corner_radius_br": radius,
            }),
            overlay: Some(true),
        })
        .await?;
    Ok(())
}

/// The Advanced Masks rectangle is measured in source pixels. A square mask
/// plus a radius of half its width is a true circle; keep the full 16:9
/// rectangle for rounded camera tiles.
fn mask_dimensions(radius: f64) -> (f64, f64) {
    if radius >= 400.0 {
        (1080.0, 1080.0)
    } else {
        (1920.0, 1080.0)
    }
}

/// List the scenes that Aitum reports to its vendor API.
pub async fn aitum_scenes(client: &Client) -> Result<Value> {
    let response: obws::responses::general::VendorResponse<Value> = client
        .general()
        .call_vendor_request(obws::requests::general::CallVendorRequest {
            vendor_name: "aitum-vertical-canvas",
            request_type: "get_scenes",
            request_data: &json!({}),
        })
        .await?;
    Ok(response.response_data)
}

/// Switch the vertical canvas through Aitum's vendor API.
pub async fn switch_vertical_scene(client: &Client, scene: &str) -> Result<Value> {
    let response: obws::responses::general::VendorResponse<Value> = client
        .general()
        .call_vendor_request(obws::requests::general::CallVendorRequest {
            vendor_name: "aitum-vertical-canvas",
            request_type: "switch_scene",
            request_data: &json!({ "scene": scene }),
        })
        .await?;
    Ok(response.response_data)
}

/// Read the current vertical scene through Aitum's vendor API.
pub async fn current_vertical_scene(client: &Client) -> Result<Value> {
    let response: obws::responses::general::VendorResponse<Value> = client
        .general()
        .call_vendor_request(obws::requests::general::CallVendorRequest {
            vendor_name: "aitum-vertical-canvas",
            request_type: "current_scene",
            request_data: &json!({}),
        })
        .await?;
    Ok(response.response_data)
}

/// Save a PNG of a source to disk.
///
/// OBS resolves relative screenshot paths against the OBS process working
/// directory, not the caller's directory, so the path is made absolute first.
pub async fn save_source_screenshot(
    client: &Client,
    source: obws::requests::sources::SourceId<'_>,
    path: &std::path::Path,
    width: u32,
    height: u32,
) -> Result<()> {
    let path = std::path::absolute(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    client
        .sources()
        .save_screenshot(SaveScreenshot {
            source,
            format: "png",
            width: Some(width),
            height: Some(height),
            compression_quality: Some(-1),
            file_path: &path,
        })
        .await
        .with_context(|| format!("saving screenshot to {}", path.display()))?;
    Ok(())
}

/// Save a PNG of a main-canvas scene source to disk.
pub async fn save_scene_screenshot(
    client: &Client,
    scene: &str,
    path: &std::path::Path,
) -> Result<()> {
    save_source_screenshot(
        client,
        obws::requests::sources::SourceId::Name(scene),
        path,
        1920,
        1080,
    )
    .await
}

/// Save a PNG of a scene by UUID, which is required when scene names are
/// duplicated across canvases.
pub async fn save_scene_screenshot_uuid(
    client: &Client,
    scene_uuid: &str,
    path: &std::path::Path,
    width: u32,
    height: u32,
) -> Result<()> {
    let path = std::path::absolute(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    client
        .raw_request(
            "SaveSourceScreenshot",
            json!({
                "sourceUuid": scene_uuid,
                "imageFormat": "png",
                "imageFilePath": path.to_string_lossy(),
                "imageWidth": width,
                "imageHeight": height,
                "imageCompressionQuality": -1,
            }),
        )
        .await
        .with_context(|| format!("saving screenshot to {}", path.display()))?;
    Ok(())
}

/// Return all scene sources in the main canvas.
pub async fn main_scenes(client: &Client) -> Result<Vec<Scene>> {
    Ok(client.scenes().list().await?.scenes)
}

/// Return scene names in the vertical canvas.
pub async fn vertical_scene_names(client: &Client) -> Result<Vec<String>> {
    let response = vertical_scene_response(client).await?;
    Ok(response
        .get("scenes")
        .and_then(Value::as_array)
        .map(|scenes| {
            scenes
                .iter()
                .filter_map(|scene| scene.get("sceneName")?.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

/// Resolve a scene name to its UUID in the Aitum vertical canvas.
///
/// Scene names are not unique across OBS canvases. `GetSourceScreenshot` by
/// `sourceName` resolves the main-canvas scene, so vertical previews must use
/// the UUID returned for the vertical canvas.
pub async fn vertical_scene_uuid(client: &Client, name: &str) -> Result<String> {
    let response = vertical_scene_response(client).await?;
    response
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| {
            scenes
                .iter()
                .find(|scene| scene.get("sceneName").and_then(Value::as_str) == Some(name))
                .and_then(|scene| scene.get("sceneUuid"))
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
        .with_context(|| format!("vertical scene {name} was not found"))
}

async fn vertical_scene_response(client: &Client) -> Result<Value> {
    let canvases = client.canvases().list().await?;
    let vertical = canvases
        .iter()
        .find(|canvas| {
            !canvas
                .flags
                .contains(obws::responses::canvases::CanvasFlags::MAIN)
        })
        .context("vertical canvas not found")?;
    client
        .raw_request(
            "GetSceneList",
            json!({ "canvasUuid": vertical.id.uuid.to_string() }),
        )
        .await
        .context("list vertical canvas scenes")
}

/// Count OBS inputs whose name starts with `Vidyst_`. Used by Test 5.
pub async fn vidyst_input_count(client: &Client) -> Result<usize> {
    Ok(client
        .inputs()
        .list(None)
        .await?
        .iter()
        .filter(|input| input.id.name.starts_with("Vidyst_"))
        .count())
}

/// Count filters on the camera wrapper. Used by Test 5.
pub async fn camera_filter_count(client: &Client) -> Result<usize> {
    Ok(client
        .filters()
        .list(obws::requests::sources::SourceId::Name(CAMERA_GROUP))
        .await?
        .len())
}
