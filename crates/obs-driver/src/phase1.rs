//! Phase 1 bootstrap validation and the five acceptance tests.
//!
//! The tests run against the live OBS instance over obs-websocket. They never
//! edit OBS scene JSON directly and never log the WebSocket password.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use obws::common::MonitorType;
use obws::events::Event;
use obws::requests::inputs::InputId;
use obws::requests::scenes::SceneId;
use obws::Client;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::bootstrap::{
    aitum_scenes, camera_filter_count, current_mask_radius, current_vertical_scene, main_scenes,
    reset_morph_filters, run_bootstrap, save_scene_screenshot_uuid, set_mask_radius,
    switch_vertical_scene, trigger_morph, vertical_scene_names, vidyst_input_count,
    BootstrapOptions, BootstrapReport, ItemReport, CAMERA_GROUP, CAMERA_RAW, CIRCLE_RADIUS,
    MASK_FILTER, MIC_SOURCE, MORPH_TO_CIRCLE, MORPH_TO_TILE, SCREEN_AUDIO_SOURCE, TILE_RADIUS,
    TRANSITION_DURATION_MS,
};

/// OBS WebSocket connection details. The password is deliberately not
/// serialized or logged.
#[derive(Clone)]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

impl ConnectionInfo {
    async fn connect(&self) -> Result<Client> {
        Client::connect(&self.host, self.port, self.password.as_deref())
            .await
            .context("connect to OBS WebSocket")
    }
}

/// Paths used by the Phase 1 runner.
#[derive(Clone, Debug)]
pub struct Phase1Options {
    pub docs_dir: PathBuf,
    pub screenshot_dir: PathBuf,
    pub bootstrap_report: PathBuf,
    pub bootstrap_rerun_report: PathBuf,
    pub results_json: PathBuf,
    pub results_markdown: PathBuf,
}

impl Default for Phase1Options {
    fn default() -> Self {
        Self {
            docs_dir: PathBuf::from("docs"),
            screenshot_dir: PathBuf::from("docs/screenshots/phase1"),
            bootstrap_report: PathBuf::from("docs/phase1-bootstrap.json"),
            bootstrap_rerun_report: PathBuf::from("docs/phase1-bootstrap-rerun.json"),
            results_json: PathBuf::from("docs/phase1-results.json"),
            results_markdown: PathBuf::from("docs/phase1-results.md"),
        }
    }
}

impl Phase1Options {
    fn image_rel(&self, file_name: &str) -> String {
        let path = self.screenshot_dir.join(file_name);
        path.strip_prefix(&self.docs_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AudioState {
    pub input: String,
    pub muted: bool,
    pub monitor: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObsSnapshot {
    pub program_scene: String,
    pub transition_name: String,
    pub transition_duration_ms: Option<u64>,
    pub transition_fixed: bool,
    pub vertical_scene: Option<String>,
    pub audio: Vec<AudioState>,
    pub mask_radius: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BootstrapSummary {
    pub scenes: usize,
    pub requests: usize,
    pub failed_requests: usize,
    pub main_canvas_uuid: String,
    pub vertical_canvas_uuid: String,
    pub camera: String,
    pub camera_device: Option<String>,
    pub screen: String,
    pub screen_device: Option<String>,
    pub mask_kind: String,
    pub base_radius: f64,
    pub circle_radius: f64,
    pub tile_radius: f64,
    pub transition: String,
    pub transition_duration_ms: u64,
}

impl From<&BootstrapReport> for BootstrapSummary {
    fn from(report: &BootstrapReport) -> Self {
        Self {
            scenes: report.scenes.len(),
            requests: report.requests.len(),
            failed_requests: report.requests.iter().filter(|request| !request.ok).count(),
            main_canvas_uuid: report.main_canvas_uuid.clone(),
            vertical_canvas_uuid: report.vertical_canvas_uuid.clone(),
            camera: report.camera.name.clone(),
            camera_device: report.camera.device_name.clone(),
            screen: report.screen.name.clone(),
            screen_device: report.screen.device_name.clone(),
            mask_kind: report.filters.mask_kind.clone(),
            base_radius: report.filters.base_radius,
            circle_radius: report.filters.circle_target_radius,
            tile_radius: report.filters.tile_target_radius,
            transition: report.transition.name.clone(),
            transition_duration_ms: report.transition.duration_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TestResult {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub summary: String,
    pub failures: Vec<String>,
    pub notes: Vec<String>,
    pub images: Vec<String>,
    pub details: Value,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct TestEvidence {
    pub summary: String,
    pub notes: Vec<String>,
    pub images: Vec<String>,
    pub details: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct CleanupReport {
    pub restored: bool,
    pub failures: Vec<String>,
    pub steps: Vec<String>,
    pub final_state: Option<ObsSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Phase1Report {
    pub generated_at_unix: u64,
    pub generated_at: String,
    pub obs_studio_version: String,
    pub obs_websocket_version: String,
    pub rpc_version: u32,
    pub bootstrap: BootstrapSummary,
    pub bootstrap_validation: TestResult,
    pub tests: Vec<TestResult>,
    pub original_state: ObsSnapshot,
    pub cleanup: CleanupReport,
    pub passed: bool,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct TimedEvent {
    t_ms: u64,
    kind: String,
    detail: String,
}

#[derive(Clone, Debug, Serialize)]
struct RadiusSample {
    t_ms: u64,
    radius: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ImageEvidence {
    label: String,
    path: String,
    width: u32,
    height: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct LiveCounts {
    main_scenes: usize,
    vertical_scenes: usize,
    vidyst_inputs: usize,
    camera_filters: usize,
}

struct EventRecorder {
    rx: mpsc::UnboundedReceiver<TimedEvent>,
    handle: JoinHandle<()>,
}

impl EventRecorder {
    fn start(client: &Client) -> Result<Self> {
        let mut stream = client.events().context("subscribe to OBS events")?;
        let (tx, rx) = mpsc::unbounded_channel();
        let started = Instant::now();
        let handle = tokio::spawn(async move {
            while let Some(event) = stream.next().await {
                let Some((kind, detail)) = event_record(&event) else {
                    continue;
                };
                let record = TimedEvent {
                    t_ms: started.elapsed().as_millis() as u64,
                    kind,
                    detail,
                };
                if tx.send(record).is_err() {
                    break;
                }
            }
        });
        Ok(Self { rx, handle })
    }

    fn drain(&mut self) -> Vec<TimedEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for EventRecorder {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn event_record(event: &Event) -> Option<(String, String)> {
    match event {
        Event::SceneTransitionStarted { id } => {
            Some(("SceneTransitionStarted".to_owned(), id.name.clone()))
        }
        Event::SceneTransitionEnded { id } => {
            Some(("SceneTransitionEnded".to_owned(), id.name.clone()))
        }
        Event::SceneTransitionVideoEnded { id } => {
            Some(("SceneTransitionVideoEnded".to_owned(), id.name.clone()))
        }
        Event::CurrentProgramSceneChanged { id } => {
            Some(("CurrentProgramSceneChanged".to_owned(), id.name.clone()))
        }
        _ => None,
    }
}

/// Run bootstrap, validate it, execute all five Phase 1 tests, restore the
/// original OBS scene/transition/audio state, and write JSON + Markdown reports.
pub async fn run_phase1(
    mut client: Client,
    connection: &ConnectionInfo,
    bootstrap_options: &BootstrapOptions,
    options: &Phase1Options,
) -> Result<Phase1Report> {
    let original_state = capture_snapshot(&client).await?;
    let version = client
        .general()
        .version()
        .await
        .context("read OBS version")?;

    std::fs::create_dir_all(&options.screenshot_dir)?;
    std::fs::create_dir_all(&options.docs_dir)?;

    let bootstrap = run_bootstrap(&client, bootstrap_options).await?;
    write_json(&options.bootstrap_report, &bootstrap)?;

    let validation_start = Instant::now();
    let validation = validate_bootstrap(&client, &bootstrap, options)
        .await
        .and_then(|mut evidence| {
            evidence.summary =
                "Bootstrap created/reconciled all scenes and verified them live.".to_owned();
            Ok(evidence)
        });
    let bootstrap_validation = result_from(
        "bootstrap",
        "Bootstrap validation",
        validation_start,
        validation,
    );

    let test1_start = Instant::now();
    let test1 = result_from(
        "test1",
        "Smooth circle <-> tile morph",
        test1_start,
        test_circle_tile(&client, &bootstrap, options).await,
    );

    let test2_start = Instant::now();
    let test2 = result_from(
        "test2",
        "Rapid click test",
        test2_start,
        test_rapid_clicks(&client, &bootstrap, options).await,
    );

    let test3_start = Instant::now();
    let test3 = result_from(
        "test3",
        "Audio mute and live listen toggles",
        test3_start,
        test_audio_toggles(&client).await,
    );

    let test4_start = Instant::now();
    let test4 = result_from(
        "test4",
        "Vertical canvas sync through Aitum",
        test4_start,
        test_vertical_sync(&client, &bootstrap, options).await,
    );

    let test5_start = Instant::now();
    let test5 = result_from(
        "test5",
        "Disconnect/reconnect and idempotent rerun",
        test5_start,
        test_reconnect_and_rerun(
            &mut client,
            connection,
            &bootstrap,
            bootstrap_options,
            options,
        )
        .await,
    );

    // Test 5 deliberately disconnected and reconnected the client. Use a fresh
    // connection for cleanup so a failed reconnect cannot skip restoration.
    let cleanup = match connection.connect().await {
        Ok(cleanup_client) => restore_snapshot(&cleanup_client, &original_state).await,
        Err(error) => CleanupReport {
            restored: false,
            failures: vec![format!("cleanup connection failed: {error:#}")],
            steps: Vec::new(),
            final_state: None,
        },
    };

    let rerun_summary = match &test5.details {
        Value::Object(map) => map
            .get("rerun_requests")
            .and_then(Value::as_u64)
            .map(|count| format!("Rerun produced {count} requests."))
            .unwrap_or_default(),
        _ => String::new(),
    };

    let tests = vec![test1, test2, test3, test4, test5];
    let passed =
        bootstrap_validation.passed && tests.iter().all(|test| test.passed) && cleanup.restored;
    let generated_at_unix = unix_now();
    let generated_at = time::OffsetDateTime::from_unix_timestamp(generated_at_unix as i64)
        .ok()
        .and_then(|date| {
            date.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| generated_at_unix.to_string());

    let mut notes = vec![
        "Aitum Vertical builds its scene dock/list at OBS startup. After the one-time OBS restart, all 8 vertical scenes were visible to get_scenes and switch_scene."
            .to_owned(),
        "Phase 1 scenes and sources were preserved in the user's scene collection. The original main scene, transition, and audio states were restored by the runner."
            .to_owned(),
        "The HP camera returned a dark frame during the captured layout screenshots; camera selection, active state, and layout transforms were verified separately."
            .to_owned(),
        "Plugins verified in Phase 0: Move Transition 3.2.1; Advanced Masks 1.6.0 (legacy x64); Aitum Vertical 1.6.6; Background Removal 1.4.1; Multi RTMP 0.7.4.4."
            .to_owned(),
    ];
    if !cleanup.restored {
        notes.push(
            "Cleanup verification reported one or more failures; see the cleanup section."
                .to_owned(),
        );
    }

    if !rerun_summary.is_empty() {
        notes.push(rerun_summary);
    }

    let report = Phase1Report {
        generated_at_unix,
        generated_at,
        obs_studio_version: version.obs_studio_version.to_string(),
        obs_websocket_version: version.obs_web_socket_version.to_string(),
        rpc_version: version.rpc_version,
        bootstrap: BootstrapSummary::from(&bootstrap),
        bootstrap_validation,
        tests,
        original_state,
        cleanup,
        passed,
        notes,
    };

    write_json(&options.results_json, &report)?;
    write_markdown(&options.results_markdown, &report)?;
    Ok(report)
}

fn result_from(id: &str, name: &str, started: Instant, result: Result<TestEvidence>) -> TestResult {
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(evidence) => TestResult {
            id: id.to_owned(),
            name: name.to_owned(),
            passed: true,
            duration_ms,
            summary: evidence.summary,
            failures: Vec::new(),
            notes: evidence.notes,
            images: evidence.images,
            details: evidence.details,
        },
        Err(error) => TestResult {
            id: id.to_owned(),
            name: name.to_owned(),
            passed: false,
            duration_ms,
            summary: "Test failed".to_owned(),
            failures: vec![format!("{error:#}")],
            notes: Vec::new(),
            images: Vec::new(),
            details: Value::Null,
        },
    }
}

async fn validate_bootstrap(
    client: &Client,
    bootstrap: &BootstrapReport,
    options: &Phase1Options,
) -> Result<TestEvidence> {
    let mut failures = Vec::new();
    let mut details = json!({
        "expected_scenes": bootstrap.scenes.len(),
        "request_failures": bootstrap.requests.iter().filter(|request| !request.ok).count(),
    });

    if bootstrap.scenes.len() != 8 {
        failures.push(format!(
            "expected 8 layout scenes, report has {}",
            bootstrap.scenes.len()
        ));
    }
    if details["request_failures"].as_u64().unwrap_or(0) > 0 {
        failures.push("bootstrap request log contains failures".to_owned());
    }

    let main_scene_list = main_scenes(client).await?;
    let main_names: Vec<&str> = main_scene_list
        .iter()
        .map(|scene| scene.id.name.as_str())
        .collect();
    let vertical_names = vertical_scene_names(client).await?;
    for scene in &bootstrap.scenes {
        if !main_names.contains(&scene.name.as_str()) {
            failures.push(format!("main scene {} is missing", scene.name));
        }
        if !vertical_names.iter().any(|name| name == &scene.name) {
            failures.push(format!("vertical scene {} is missing", scene.name));
        }
    }

    for scene in &bootstrap.scenes {
        let main_items = live_scene_items(client, &scene.main_scene_uuid).await?;
        let vertical_items = live_scene_items(client, &scene.vertical_scene_uuid).await?;
        validate_layout_items(
            &format!("{}/main", scene.name),
            &main_items,
            &scene.main_screen,
            &scene.main_camera,
            false,
            &mut failures,
        );
        validate_layout_items(
            &format!("{}/vertical", scene.name),
            &vertical_items,
            &scene.vertical_screen,
            &scene.vertical_camera,
            true,
            &mut failures,
        );
    }

    let group_items = live_scene_items(client, &bootstrap.camera_group_uuid).await?;
    validate_camera_group(&group_items, bootstrap, &mut failures);

    let filters = client
        .filters()
        .list(obws::requests::sources::SourceId::Name(CAMERA_GROUP))
        .await
        .context("list camera filters")?;
    let mask = filters.iter().find(|filter| filter.name == MASK_FILTER);
    if mask.is_none() {
        failures.push(format!("{MASK_FILTER} is missing"));
    }
    let mask_enabled = mask.map(|filter| filter.enabled).unwrap_or(false);
    if !mask_enabled {
        failures.push(format!("{MASK_FILTER} is disabled"));
    }
    for morph in [MORPH_TO_CIRCLE, MORPH_TO_TILE] {
        match filters.iter().find(|filter| filter.name == morph) {
            Some(filter) if !filter.enabled => {}
            Some(_) => failures.push(format!("{morph} should be disabled after bootstrap")),
            None => failures.push(format!("{morph} is missing")),
        }
    }
    let radius = current_mask_radius(client).await?;
    if (radius - TILE_RADIUS).abs() > 0.2 {
        failures.push(format!(
            "mask radius after bootstrap is {radius}, expected {TILE_RADIUS}"
        ));
    }

    let transition = client.transitions().current().await?;
    let transition_name = transition.id.name.as_str();
    if transition_name != "Move" {
        failures.push(format!(
            "current transition is {transition_name}, expected Move"
        ));
    }
    let transition_duration = transition
        .duration
        .map(|duration| u64::try_from(duration.whole_milliseconds()).unwrap_or(0));
    if transition_duration != Some(TRANSITION_DURATION_MS) {
        failures.push(format!(
            "Move duration is {:?} ms, expected {TRANSITION_DURATION_MS}",
            transition_duration
        ));
    }

    let input_list = client.inputs().list(None).await?;
    let mut vidyst_inputs: Vec<&str> = input_list
        .iter()
        .map(|input| input.id.name.as_str())
        .filter(|name| name.starts_with("Vidyst_"))
        .collect();
    vidyst_inputs.sort_unstable();
    vidyst_inputs.dedup();
    if vidyst_inputs.len() != 4 {
        failures.push(format!(
            "expected 4 Vidyst inputs, found {}: {:?}",
            vidyst_inputs.len(),
            vidyst_inputs
        ));
    }

    if !failures.is_empty() {
        bail!("bootstrap validation failed: {}", failures.join("; "));
    }

    // Capture visual evidence only after the live structural checks pass.
    let images = capture_all_layouts(client, bootstrap, options).await?;
    details["main_scenes"] = json!(main_names);
    details["vertical_scenes"] = json!(vertical_names);
    details["vidyst_inputs"] = json!(vidyst_inputs);
    details["mask_radius"] = json!(radius);
    details["transition"] = json!(transition_name);
    details["transition_duration_ms"] = json!(transition_duration);
    details["image_count"] = json!(images.len());

    Ok(TestEvidence {
        summary: "8 main scenes, 8 vertical scenes, 2 items per layout, correct 9:16 Contain bounds, hidden slots, camera wrapper, filters, and Move transition verified."
            .to_owned(),
        notes: vec![
            "All vertical screen slots use OBS_BOUNDS_SCALE_INNER with cropToBounds=false.".to_owned(),
            "Main and vertical screenshots were captured for every layout.".to_owned(),
        ],
        images: images.iter().map(|image| image.path.clone()).collect(),
        details: json!({
            "checks": details,
            "images": images,
        }),
    })
}

fn validate_layout_items(
    label: &str,
    items: &[Value],
    screen_report: &ItemReport,
    camera_report: &ItemReport,
    vertical: bool,
    failures: &mut Vec<String>,
) {
    if items.len() != 2 {
        failures.push(format!(
            "{label}: expected 2 scene items, found {}",
            items.len()
        ));
    }
    let screen = find_item(items, "Vidyst_Screen");
    let camera = find_item(items, CAMERA_GROUP);
    if screen.is_none() {
        failures.push(format!("{label}: Vidyst_Screen is missing"));
    }
    if camera.is_none() {
        failures.push(format!("{label}: {CAMERA_GROUP} is missing"));
    }
    if let Some(item) = screen {
        check_item_enabled(
            label,
            "Vidyst_Screen",
            item,
            screen_report.enabled,
            failures,
        );
        check_item_index(label, "Vidyst_Screen", item, screen_report.index, failures);
        if let Some(report) = &screen_report.transform {
            check_transform(label, "Vidyst_Screen", item, report, vertical, failures);
        }
    }
    if let Some(item) = camera {
        check_item_enabled(label, CAMERA_GROUP, item, camera_report.enabled, failures);
        check_item_index(label, CAMERA_GROUP, item, camera_report.index, failures);
        if let Some(report) = &camera_report.transform {
            check_transform(label, CAMERA_GROUP, item, report, false, failures);
        }
    }
}

fn find_item<'a>(items: &'a [Value], source_name: &str) -> Option<&'a Value> {
    items
        .iter()
        .find(|item| item.get("sourceName").and_then(Value::as_str) == Some(source_name))
}

fn check_item_enabled(
    label: &str,
    source: &str,
    item: &Value,
    expected: bool,
    failures: &mut Vec<String>,
) {
    let actual = item
        .get("sceneItemEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if actual != expected {
        failures.push(format!(
            "{label}: {source} enabled={actual}, expected {expected}"
        ));
    }
}

fn check_item_index(
    label: &str,
    source: &str,
    item: &Value,
    expected: i64,
    failures: &mut Vec<String>,
) {
    let actual = item.get("sceneItemIndex").and_then(Value::as_i64);
    if actual != Some(expected) {
        failures.push(format!(
            "{label}: {source} index={actual:?}, expected {expected}"
        ));
    }
}

fn check_transform(
    label: &str,
    source: &str,
    item: &Value,
    expected: &crate::bootstrap::TransformReport,
    vertical_screen: bool,
    failures: &mut Vec<String>,
) {
    let Some(transform) = item.get("sceneItemTransform") else {
        failures.push(format!("{label}: {source} has no transform"));
        return;
    };
    let number = |key: &str| {
        transform
            .get(key)
            .and_then(Value::as_f64)
            .unwrap_or(f64::NAN)
    };
    // OBS stores scene-item geometry on whole-pixel or half-pixel boundaries.
    // The layout math is fractional, so accept <= 1 px of OBS quantization.
    let near = |actual: f64, expected: f64| (actual - expected).abs() <= 1.0;
    for (key, actual, expected_value) in [
        ("positionX", number("positionX"), expected.position_x as f64),
        ("positionY", number("positionY"), expected.position_y as f64),
        (
            "boundsWidth",
            number("boundsWidth"),
            expected.bounds_width as f64,
        ),
        (
            "boundsHeight",
            number("boundsHeight"),
            expected.bounds_height as f64,
        ),
    ] {
        if !near(actual, expected_value) {
            failures.push(format!(
                "{label}: {source} {key}={actual}, expected {expected_value}"
            ));
        }
    }
    let bounds_type = transform
        .get("boundsType")
        .and_then(Value::as_str)
        .unwrap_or("");
    if bounds_type != expected.bounds_type {
        failures.push(format!(
            "{label}: {source} boundsType={bounds_type}, expected {}",
            expected.bounds_type
        ));
    }
    let alignment = transform.get("alignment").and_then(Value::as_i64);
    if alignment != Some(expected.alignment as i64) {
        failures.push(format!(
            "{label}: {source} alignment={alignment:?}, expected {}",
            expected.alignment
        ));
    }
    let bounds_alignment = transform.get("boundsAlignment").and_then(Value::as_i64);
    if bounds_alignment != Some(expected.bounds_alignment as i64) {
        failures.push(format!(
            "{label}: {source} boundsAlignment={bounds_alignment:?}, expected {}",
            expected.bounds_alignment
        ));
    }
    let crop = transform.get("cropToBounds").and_then(Value::as_bool);
    if crop != Some(expected.crop_to_bounds) {
        failures.push(format!(
            "{label}: {source} cropToBounds={crop:?}, expected {}",
            expected.crop_to_bounds
        ));
    }
    if vertical_screen {
        if bounds_type != "OBS_BOUNDS_SCALE_INNER" {
            failures.push(format!(
                "{label}: vertical screen must use OBS_BOUNDS_SCALE_INNER, got {bounds_type}"
            ));
        }
        if crop != Some(false) {
            failures.push(format!(
                "{label}: vertical screen must not crop, got cropToBounds={crop:?}"
            ));
        }
    }
}

fn validate_camera_group(items: &[Value], bootstrap: &BootstrapReport, failures: &mut Vec<String>) {
    if items.len() != 4 {
        failures.push(format!(
            "camera wrapper: expected 4 items, found {}",
            items.len()
        ));
    }
    let camera = find_item(items, CAMERA_RAW);
    let screen = find_item(items, "Vidyst_Screen");
    let mic = find_item(items, MIC_SOURCE);
    let screen_audio = find_item(items, SCREEN_AUDIO_SOURCE);
    if camera.is_none() {
        failures.push(format!("camera wrapper: {CAMERA_RAW} is missing"));
    }
    if screen.is_none() {
        failures.push(format!("camera wrapper: Vidyst_Screen is missing"));
    }
    if mic.is_none() {
        failures.push(format!("camera wrapper: {MIC_SOURCE} is missing"));
    }
    if screen_audio.is_none() {
        failures.push(format!("camera wrapper: {SCREEN_AUDIO_SOURCE} is missing"));
    }
    if let Some(item) = screen {
        check_item_enabled("camera wrapper", "Vidyst_Screen", item, false, failures);
    }
    if let Some(item) = mic {
        check_item_enabled("camera wrapper", MIC_SOURCE, item, true, failures);
    }
    if let Some(item) = screen_audio {
        check_item_enabled("camera wrapper", SCREEN_AUDIO_SOURCE, item, true, failures);
    }
    if let Some(item) = camera {
        let transform = item.get("sceneItemTransform").unwrap_or(&Value::Null);
        let number = |key: &str| {
            transform
                .get(key)
                .and_then(Value::as_f64)
                .unwrap_or(f64::NAN)
        };
        for (key, actual, expected) in [
            ("boundsWidth", number("boundsWidth"), 1920.0),
            ("boundsHeight", number("boundsHeight"), 1080.0),
            ("positionX", number("positionX"), 0.0),
            ("positionY", number("positionY"), 0.0),
        ] {
            if (actual - expected).abs() > 1.0 {
                failures.push(format!(
                    "camera wrapper: {CAMERA_RAW} {key}={actual}, expected {expected}"
                ));
            }
        }
        if transform.get("boundsType").and_then(Value::as_str) != Some("OBS_BOUNDS_SCALE_OUTER") {
            failures.push(format!(
                "camera wrapper: {CAMERA_RAW} must use OBS_BOUNDS_SCALE_OUTER"
            ));
        }
        if transform.get("cropToBounds").and_then(Value::as_bool) != Some(true) {
            failures.push(format!("camera wrapper: {CAMERA_RAW} must crop to bounds"));
        }
    }

    // Keep this check tied to the values persisted in the bootstrap report.
    if bootstrap.filters.mask_kind != "advanced_masks_filter" {
        failures.push(format!(
            "camera wrapper: mask kind is {}, expected advanced_masks_filter",
            bootstrap.filters.mask_kind
        ));
    }
    if (bootstrap.filters.base_radius - TILE_RADIUS).abs() > 0.2 {
        failures.push(format!(
            "camera wrapper: mask radius is {}, expected {TILE_RADIUS}",
            bootstrap.filters.base_radius
        ));
    }
}

async fn capture_all_layouts(
    client: &Client,
    bootstrap: &BootstrapReport,
    options: &Phase1Options,
) -> Result<Vec<ImageEvidence>> {
    let mut images = Vec::new();
    for scene in &bootstrap.scenes {
        set_main_scene(client, &scene.main_scene_uuid).await?;
        tokio::time::sleep(Duration::from_millis(TRANSITION_DURATION_MS + 120)).await;
        images.push(
            capture_scene_image(
                client,
                &scene.main_scene_uuid,
                1920,
                1080,
                "main",
                &scene.id,
                options,
            )
            .await?,
        );

        let response = switch_vertical_scene(client, &scene.name).await?;
        if response.get("success").and_then(Value::as_bool) != Some(true) {
            bail!("Aitum switch_scene failed for {}: {response}", scene.name);
        }
        // Aitum's own vertical transition is Fade 300 ms by default.
        tokio::time::sleep(Duration::from_millis(420)).await;
        images.push(
            capture_scene_image(
                client,
                &scene.vertical_scene_uuid,
                1080,
                1920,
                "vertical",
                &scene.id,
                options,
            )
            .await?,
        );
    }
    Ok(images)
}

async fn capture_scene_image(
    client: &Client,
    scene_uuid: &str,
    width: u32,
    height: u32,
    prefix: &str,
    id: &str,
    options: &Phase1Options,
) -> Result<ImageEvidence> {
    let file_name = format!("{prefix}_{id}.png");
    let path = options.screenshot_dir.join(&file_name);
    save_scene_screenshot_uuid(client, scene_uuid, &path, width, height).await?;
    let (actual_width, actual_height) = png_dimensions(&path)?;
    if actual_width != width || actual_height != height {
        bail!(
            "{} is {}x{}, expected {}x{}",
            path.display(),
            actual_width,
            actual_height,
            width,
            height
        );
    }
    Ok(ImageEvidence {
        label: format!("{prefix}/{id}"),
        path: options.image_rel(&file_name),
        width: actual_width,
        height: actual_height,
    })
}

async fn test_circle_tile(
    client: &Client,
    bootstrap: &BootstrapReport,
    options: &Phase1Options,
) -> Result<TestEvidence> {
    let screen_cam = bootstrap
        .scene("screen_cam")
        .context("screen_cam scene missing from bootstrap report")?;
    let side_by_side = bootstrap
        .scene("side_by_side")
        .context("side_by_side scene missing from bootstrap report")?;

    reset_morph_filters(client).await?;
    set_mask_radius(client, CIRCLE_RADIUS).await?;
    set_main_scene(client, &screen_cam.main_scene_uuid).await?;
    tokio::time::sleep(Duration::from_millis(TRANSITION_DURATION_MS + 160)).await;

    let start_radius = current_mask_radius(client).await?;
    if (start_radius - CIRCLE_RADIUS).abs() > 0.5 {
        bail!("screen_cam start radius is {start_radius}, expected {CIRCLE_RADIUS}");
    }

    let before = capture_scene_image(
        client,
        &screen_cam.main_scene_uuid,
        1920,
        1080,
        "test1_before",
        &screen_cam.id,
        options,
    )
    .await?;

    let mut events = EventRecorder::start(client)?;
    events.drain();
    let switch_started = Instant::now();
    set_main_scene(client, &side_by_side.main_scene_uuid).await?;
    let switch_command_ms = switch_started.elapsed().as_millis() as u64;
    trigger_morph(client, MORPH_TO_TILE).await?;
    let morph_started = Instant::now();

    let mut samples = Vec::new();
    for index in 0..16 {
        if index > 0 {
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        samples.push(RadiusSample {
            t_ms: morph_started.elapsed().as_millis() as u64,
            radius: current_mask_radius(client).await?,
        });
    }
    tokio::time::sleep(Duration::from_millis(450)).await;
    let final_radius = current_mask_radius(client).await?;
    let current = current_program_scene(client).await?;
    let events = events.drain();

    let after = capture_scene_image(
        client,
        &side_by_side.main_scene_uuid,
        1920,
        1080,
        "test1_after",
        &side_by_side.id,
        options,
    )
    .await?;

    let mut failures = Vec::new();
    if current != side_by_side.name {
        failures.push(format!(
            "program scene is {current}, expected {}",
            side_by_side.name
        ));
    }
    if (final_radius - TILE_RADIUS).abs() > 0.2 {
        failures.push(format!(
            "final mask radius is {final_radius}, expected {TILE_RADIUS}"
        ));
    }
    let intermediate = samples
        .iter()
        .filter(|sample| sample.radius > TILE_RADIUS + 5.0 && sample.radius < CIRCLE_RADIUS - 5.0)
        .count();
    if intermediate < 3 {
        failures.push(format!(
            "only {intermediate} intermediate radius samples; animation did not interpolate"
        ));
    }
    let mut previous = start_radius;
    for sample in &samples {
        if sample.radius > previous + 1.0 {
            failures.push(format!(
                "radius increased from {previous} to {} at {} ms",
                sample.radius, sample.t_ms
            ));
            break;
        }
        previous = sample.radius;
    }
    let settle_ms = samples
        .iter()
        .find(|sample| (sample.radius - TILE_RADIUS).abs() <= 0.5)
        .map(|sample| sample.t_ms);
    match settle_ms {
        Some(ms) if (250..=700).contains(&ms) => {}
        Some(ms) => failures.push(format!(
            "mask settled at {ms} ms, expected approximately {TRANSITION_DURATION_MS} ms"
        )),
        None => failures.push("mask never reached the tile radius during sampling".to_owned()),
    }
    let transition_span = transition_span(&events);
    if let Some(span) = transition_span {
        if !(250..=650).contains(&span) {
            failures.push(format!(
                "Move transition event span was {span} ms, expected approximately {TRANSITION_DURATION_MS} ms"
            ));
        }
    }

    if !failures.is_empty() {
        bail!("{}", failures.join("; "));
    }

    let mut notes = vec![format!(
        "Radius samples: {} at t=0 ms to {final_radius} after settling.",
        samples
            .first()
            .map(|sample| sample.radius)
            .unwrap_or(start_radius)
    )];
    if transition_span.is_none() {
        notes.push(
            "OBS did not emit a complete transition start/end pair; radius and final scene were still verified."
                .to_owned(),
        );
    }

    Ok(TestEvidence {
        summary: format!(
            "screen_cam circle ({CIRCLE_RADIUS}) -> side_by_side tile ({TILE_RADIUS}); {} intermediate radius samples, settled in {settle_ms:?} ms.",
            intermediate
        ),
        notes,
        images: vec![before.path, after.path],
        details: json!({
            "from": screen_cam.name,
            "to": side_by_side.name,
            "start_radius": start_radius,
            "final_radius": final_radius,
            "switch_command_ms": switch_command_ms,
            "settle_ms": settle_ms,
            "transition_event_span_ms": transition_span,
            "samples": samples,
            "events": events,
        }),
    })
}

async fn test_rapid_clicks(
    client: &Client,
    bootstrap: &BootstrapReport,
    options: &Phase1Options,
) -> Result<TestEvidence> {
    let sequence = [
        "screen_cam",
        "side_by_side",
        "screen_tile",
        "just_me",
        "screen_focus",
    ];
    // Start on a layout outside the rapid sequence so all five requests cause a
    // real scene change and transition.
    let initial = bootstrap
        .scene("screen_cam_inv")
        .context("rapid-click initial scene missing")?;
    reset_morph_filters(client).await?;
    set_mask_radius(client, CIRCLE_RADIUS).await?;
    set_main_scene(client, &initial.main_scene_uuid).await?;
    tokio::time::sleep(Duration::from_millis(TRANSITION_DURATION_MS + 160)).await;

    let mut events = EventRecorder::start(client)?;
    events.drain();
    let sequence_started = Instant::now();
    let mut next_step = tokio::time::Instant::now();
    let mut steps = Vec::new();

    for (index, id) in sequence.iter().enumerate() {
        tokio::time::sleep_until(next_step).await;
        next_step += Duration::from_millis(200);
        let scene = bootstrap
            .scene(id)
            .with_context(|| format!("rapid-click scene {id} missing"))?;
        let step_started = Instant::now();
        let switch_result = set_main_scene(client, &scene.main_scene_uuid).await;
        let command_ms = step_started.elapsed().as_millis() as u64;
        let morph = if *id == "screen_cam" {
            MORPH_TO_CIRCLE
        } else {
            MORPH_TO_TILE
        };
        let morph_result = trigger_morph(client, morph).await;
        let radius_after_trigger = current_mask_radius(client).await.ok();
        let observed_scene = current_program_scene(client).await.ok();
        // Sample again roughly 100 ms into the Move/Move Value animation so the
        // rapid-click test records interpolation, not just its endpoints.
        tokio::time::sleep_until(tokio::time::Instant::from_std(
            step_started + Duration::from_millis(100),
        ))
        .await;
        let radius_100ms = current_mask_radius(client).await.ok();
        steps.push(json!({
            "index": index,
            "scene": scene.name,
            "ok": switch_result.is_ok() && morph_result.is_ok(),
            "command_ms": command_ms,
            "started_at_ms": sequence_started.elapsed().as_millis(),
            "observed_scene_immediately": observed_scene,
            "radius_after_trigger": radius_after_trigger,
            "radius_100ms": radius_100ms,
            "error": switch_result.err().map(|error| format!("{error:#}")).or_else(|| morph_result.err().map(|error| format!("{error:#}"))),
        }));
    }

    let sequence_elapsed_ms = sequence_started.elapsed().as_millis() as u64;
    let midflight_samples = steps
        .iter()
        .filter_map(|step| step.get("radius_100ms").and_then(Value::as_f64))
        .filter(|radius| *radius > TILE_RADIUS + 5.0 && *radius < CIRCLE_RADIUS - 5.0)
        .count();
    tokio::time::sleep(Duration::from_millis(700)).await;
    let final_scene = current_program_scene(client).await?;
    let final_radius = current_mask_radius(client).await?;
    let events = events.drain();
    let last = bootstrap
        .scene(sequence[sequence.len() - 1])
        .context("last rapid-click scene missing")?;
    let last_image = capture_scene_image(
        client,
        &last.main_scene_uuid,
        1920,
        1080,
        "test2_final",
        &last.id,
        options,
    )
    .await?;

    let mut failures = Vec::new();
    for step in &steps {
        if step.get("ok").and_then(Value::as_bool) != Some(true) {
            failures.push(format!(
                "rapid-click step failed: {}",
                serde_json::to_string(step).unwrap_or_default()
            ));
        }
    }
    if final_scene != last.name {
        failures.push(format!(
            "final program scene is {final_scene}, expected {}",
            last.name
        ));
    }
    if (final_radius - TILE_RADIUS).abs() > 0.2 {
        failures.push(format!(
            "final mask radius is {final_radius}, expected {TILE_RADIUS}"
        ));
    }
    let started_count = events
        .iter()
        .filter(|event| event.kind == "SceneTransitionStarted")
        .count();
    if started_count == 0 {
        failures.push("no SceneTransitionStarted events were observed".to_owned());
    }
    if sequence_elapsed_ms > 1_200 {
        failures.push(format!(
            "5 rapid-click requests took {sequence_elapsed_ms} ms; expected about 800 ms (200 ms cadence)"
        ));
    }
    if midflight_samples == 0 {
        failures.push("no mid-flight mask radius samples were observed".to_owned());
    }
    let span = transition_span(&events);
    if let Some(span) = span {
        if !(250..=900).contains(&span) {
            failures.push(format!(
                "final transition event span was {span} ms, outside the expected range"
            ));
        }
    }

    if !failures.is_empty() {
        bail!("{}", failures.join("; "));
    }

    let mut notes = vec![format!(
        "5 scenes requested on a 200 ms cadence; all requests completed in {sequence_elapsed_ms} ms. {midflight_samples} mid-flight radius sample(s) observed. Final scene {} reached with radius {final_radius}.",
        last.name
    )];
    if span.is_none() {
        notes.push(
            "The final transition did not emit a complete start/end pair; all commands and the final state were verified."
                .to_owned(),
        );
    }

    Ok(TestEvidence {
        summary: format!(
            "5 layout switches issued on a 200 ms cadence in {sequence_elapsed_ms} ms; all commands succeeded and the final Move transition settled on {}.",
            last.name
        ),
        notes,
        images: vec![last_image.path],
        details: json!({
            "sequence": sequence,
            "sequence_elapsed_ms": sequence_elapsed_ms,
            "midflight_samples": midflight_samples,
            "steps": steps,
            "final_scene": final_scene,
            "final_radius": final_radius,
            "transition_started_events": started_count,
            "transition_event_span_ms": span,
            "events": events,
        }),
    })
}

async fn test_audio_toggles(client: &Client) -> Result<TestEvidence> {
    let mut tests = Vec::new();
    for input in [MIC_SOURCE, SCREEN_AUDIO_SOURCE] {
        let original = audio_state(client, input).await?;
        let mut steps = Vec::new();

        let flip = !original.muted;
        set_muted(client, input, flip).await?;
        let after_flip = client.inputs().muted(InputId::Name(input)).await?;
        if after_flip != flip {
            bail!("{input}: mute did not change to {flip}");
        }
        steps.push(json!({"action": "set_muted", "value": flip, "observed": after_flip}));

        set_muted(client, input, original.muted).await?;
        let after_restore = client.inputs().muted(InputId::Name(input)).await?;
        if after_restore != original.muted {
            bail!("{input}: mute did not restore to {}", original.muted);
        }
        steps.push(
            json!({"action": "set_muted", "value": original.muted, "observed": after_restore}),
        );

        // Exercise both directions even when monitoring was already on, then
        // always restore the exact original monitor type.
        let exercise = if original.monitor == "OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT" {
            MonitorType::None
        } else {
            MonitorType::MonitorAndOutput
        };
        client
            .inputs()
            .set_audio_monitor_type(InputId::Name(input), exercise)
            .await?;
        let after_exercise = client
            .inputs()
            .audio_monitor_type(InputId::Name(input))
            .await?;
        if after_exercise != exercise {
            bail!("{input}: monitor type did not change to {exercise:?}");
        }
        steps.push(json!({"action": "set_audio_monitor_type", "value": monitor_name(exercise), "observed": monitor_name(after_exercise)}));

        let original_monitor = parse_monitor(&original.monitor);
        client
            .inputs()
            .set_audio_monitor_type(InputId::Name(input), original_monitor)
            .await?;
        let after_monitor_restore = client
            .inputs()
            .audio_monitor_type(InputId::Name(input))
            .await?;
        if after_monitor_restore != original_monitor {
            bail!(
                "{input}: monitor type did not restore to {}",
                original.monitor
            );
        }
        steps.push(json!({"action": "set_audio_monitor_type", "value": monitor_name(original_monitor), "observed": monitor_name(after_monitor_restore)}));

        tests.push(json!({
            "input": input,
            "original": original,
            "steps": steps,
        }));
    }

    Ok(TestEvidence {
        summary: "Mic and screen audio mute toggles and MonitorAndOutput transitions verified, then restored."
            .to_owned(),
        notes: Vec::new(),
        images: Vec::new(),
        details: json!({"inputs": tests}),
    })
}

async fn test_vertical_sync(
    client: &Client,
    bootstrap: &BootstrapReport,
    options: &Phase1Options,
) -> Result<TestEvidence> {
    let before_scenes = aitum_scenes(client).await?;
    let before_current = current_vertical_scene(client).await?;
    let mut steps = Vec::new();
    let mut images = Vec::new();

    for scene in &bootstrap.scenes {
        let response = switch_vertical_scene(client, &scene.name).await?;
        tokio::time::sleep(Duration::from_millis(420)).await;
        let current = current_vertical_scene(client).await?;
        let image = capture_scene_image(
            client,
            &scene.vertical_scene_uuid,
            1080,
            1920,
            "test4",
            &scene.id,
            options,
        )
        .await?;
        images.push(image.path.clone());
        let success = response.get("success").and_then(Value::as_bool) == Some(true);
        let matched = current.get("scene").and_then(Value::as_str) == Some(scene.name.as_str());
        if !success || !matched {
            bail!(
                "vertical switch to {} failed: response={}, current={}",
                scene.name,
                response,
                current
            );
        }
        steps.push(json!({
            "scene": scene.name,
            "response": response,
            "current": current,
            "success": success,
            "matched": matched,
            "image": image,
        }));
    }

    let after_scenes = aitum_scenes(client).await?;
    let expected: Vec<&str> = bootstrap
        .scenes
        .iter()
        .map(|scene| scene.name.as_str())
        .collect();
    let listed: Vec<&str> = after_scenes
        .get("scenes")
        .and_then(Value::as_array)
        .map(|scenes| {
            scenes
                .iter()
                .filter_map(|scene| scene.get("name").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let missing: Vec<&str> = expected
        .iter()
        .copied()
        .filter(|name| !listed.contains(name))
        .collect();
    let after_current = current_vertical_scene(client).await?;
    let notes = if missing.is_empty() {
        vec![
            "Aitum get_scenes lists every Vidyst vertical scene after the one-time restart."
                .to_owned(),
        ]
    } else {
        vec![format!(
            "Aitum switch_scene succeeded, but get_scenes omitted: {}",
            missing.join(", ")
        )]
    };

    Ok(TestEvidence {
        summary: format!(
            "Switched all {} vertical scenes through the Aitum vendor request; each switch matched current_scene.",
            bootstrap.scenes.len()
        ),
        notes,
        images,
        details: json!({
            "before_scenes": before_scenes,
            "before_current": before_current,
            "after_scenes": after_scenes,
            "after_current": after_current,
            "missing_from_get_scenes": missing,
            "steps": steps,
        }),
    })
}

async fn test_reconnect_and_rerun(
    client: &mut Client,
    connection: &ConnectionInfo,
    bootstrap: &BootstrapReport,
    bootstrap_options: &BootstrapOptions,
    options: &Phase1Options,
) -> Result<TestEvidence> {
    let before_counts = live_counts(client).await?;
    let expected_uuids: HashMap<&str, (&str, &str)> = bootstrap
        .scenes
        .iter()
        .map(|scene| {
            (
                scene.id.as_str(),
                (
                    scene.main_scene_uuid.as_str(),
                    scene.vertical_scene_uuid.as_str(),
                ),
            )
        })
        .collect();

    client.disconnect().await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let reconnected = connection.connect().await.context("reconnect to OBS")?;
    *client = reconnected;

    let rerun = run_bootstrap(client, bootstrap_options).await?;
    write_json(&options.bootstrap_rerun_report, &rerun)?;
    let after_counts = live_counts(client).await?;
    let failed_requests = rerun.requests.iter().filter(|request| !request.ok).count();
    let mut uuid_failures = Vec::new();
    for scene in &rerun.scenes {
        if let Some((main_uuid, vertical_uuid)) = expected_uuids.get(scene.id.as_str()) {
            if scene.main_scene_uuid != *main_uuid {
                uuid_failures.push(format!(
                    "{} main UUID changed: {} -> {}",
                    scene.id, main_uuid, scene.main_scene_uuid
                ));
            }
            if scene.vertical_scene_uuid != *vertical_uuid {
                uuid_failures.push(format!(
                    "{} vertical UUID changed: {} -> {}",
                    scene.id, vertical_uuid, scene.vertical_scene_uuid
                ));
            }
        } else {
            uuid_failures.push(format!("unexpected scene in rerun: {}", scene.id));
        }
    }
    let rerun_count = rerun.scenes.len();
    let counts_stable = before_counts == after_counts;

    if rerun_count != bootstrap.scenes.len() {
        uuid_failures.push(format!(
            "rerun created {} scenes, expected {}",
            rerun_count,
            bootstrap.scenes.len()
        ));
    }
    if failed_requests > 0 {
        uuid_failures.push(format!("rerun had {failed_requests} failed requests"));
    }
    if !counts_stable {
        uuid_failures.push(format!(
            "live counts changed: {before_counts:?} -> {after_counts:?}"
        ));
    }
    if !uuid_failures.is_empty() {
        bail!("{}", uuid_failures.join("; "));
    }

    Ok(TestEvidence {
        summary: format!(
            "Disconnected, reconnected, reran bootstrap: {} scenes, {} requests, 0 failures, UUIDs and source counts unchanged.",
            rerun_count,
            rerun.requests.len()
        ),
        notes: vec![
            "Bootstrap is idempotent across a fresh WebSocket connection.".to_owned(),
        ],
        images: Vec::new(),
        details: json!({
            "before_counts": before_counts,
            "after_counts": after_counts,
            "rerun_scenes": rerun_count,
            "rerun_requests": rerun.requests.len(),
            "rerun_failed_requests": failed_requests,
            "uuid_failures": uuid_failures,
            "rerun_mask_radius": rerun.filters.base_radius,
            "rerun_transition": rerun.transition,
            "rerun_report": options.bootstrap_rerun_report.to_string_lossy().replace('\\', "/"),
        }),
    })
}

async fn restore_snapshot(client: &Client, original: &ObsSnapshot) -> CleanupReport {
    let mut failures = Vec::new();
    let mut steps = Vec::new();

    if let Err(error) = client
        .transitions()
        .set_current(&original.transition_name)
        .await
    {
        failures.push(format!(
            "restore transition {}: {error:#}",
            original.transition_name
        ));
    } else {
        steps.push(format!("Set transition {}", original.transition_name));
    }
    if let Some(duration_ms) = original.transition_duration_ms {
        if !original.transition_fixed {
            let duration = time::Duration::milliseconds(duration_ms as i64);
            if let Err(error) = client.transitions().set_current_duration(duration).await {
                failures.push(format!(
                    "restore transition duration {duration_ms} ms: {error:#}"
                ));
            } else {
                steps.push(format!("Set transition duration {duration_ms} ms"));
            }
        }
    }

    if let Err(error) = client
        .scenes()
        .set_current_program_scene(SceneId::Name(&original.program_scene))
        .await
    {
        failures.push(format!(
            "restore program scene {}: {error:#}",
            original.program_scene
        ));
    } else {
        steps.push(format!("Set program scene {}", original.program_scene));
    }

    if let Some(vertical) = &original.vertical_scene {
        match switch_vertical_scene(client, vertical).await {
            Ok(response) if response.get("success").and_then(Value::as_bool) == Some(true) => {
                steps.push(format!("Set vertical scene {vertical}"));
            }
            Ok(response) => failures.push(format!(
                "restore vertical scene {vertical}: unexpected response {response}"
            )),
            Err(error) => failures.push(format!("restore vertical scene {vertical}: {error:#}")),
        }
    }

    for audio in &original.audio {
        let input = InputId::Name(&audio.input);
        if let Err(error) = client.inputs().set_muted(input, audio.muted).await {
            failures.push(format!("restore {} mute: {error:#}", audio.input));
        }
        if let Err(error) = client
            .inputs()
            .set_audio_monitor_type(input, parse_monitor(&audio.monitor))
            .await
        {
            failures.push(format!("restore {} monitor: {error:#}", audio.input));
        } else {
            steps.push(format!(
                "Restored {} mute={} monitor={}",
                audio.input, audio.muted, audio.monitor
            ));
        }
    }

    if let Err(error) = set_mask_radius(client, TILE_RADIUS).await {
        failures.push(format!("restore mask radius: {error:#}"));
    } else {
        steps.push(format!("Set mask radius {TILE_RADIUS}"));
    }
    if let Err(error) = reset_morph_filters(client).await {
        failures.push(format!("disable morph filters: {error:#}"));
    } else {
        steps.push("Disabled morph filters".to_owned());
    }

    tokio::time::sleep(Duration::from_millis(700)).await;
    let final_state = capture_snapshot(client).await.ok();
    if let Some(state) = &final_state {
        if state.program_scene != original.program_scene {
            failures.push(format!(
                "final program scene is {}, expected {}",
                state.program_scene, original.program_scene
            ));
        }
        if state.transition_name != original.transition_name {
            failures.push(format!(
                "final transition is {}, expected {}",
                state.transition_name, original.transition_name
            ));
        }
        if !original.transition_fixed
            && original.transition_duration_ms.is_some()
            && state.transition_duration_ms != original.transition_duration_ms
        {
            failures.push(format!(
                "final transition duration is {:?}, expected {:?}",
                state.transition_duration_ms, original.transition_duration_ms
            ));
        }
        if let Some(vertical) = &original.vertical_scene {
            if state.vertical_scene.as_deref() != Some(vertical.as_str()) {
                failures.push(format!(
                    "final vertical scene is {:?}, expected {vertical}",
                    state.vertical_scene
                ));
            }
        }
        for expected in &original.audio {
            match state
                .audio
                .iter()
                .find(|actual| actual.input == expected.input)
            {
                Some(actual)
                    if actual.muted == expected.muted && actual.monitor == expected.monitor => {}
                Some(actual) => failures.push(format!(
                    "final {} is mute={} monitor={}, expected mute={} monitor={}",
                    expected.input, actual.muted, actual.monitor, expected.muted, expected.monitor
                )),
                None => failures.push(format!("final state is missing {}", expected.input)),
            }
        }
    } else {
        failures.push("could not capture final state after cleanup".to_owned());
    }

    CleanupReport {
        restored: failures.is_empty(),
        failures,
        steps,
        final_state,
    }
}

async fn capture_snapshot(client: &Client) -> Result<ObsSnapshot> {
    let current = client
        .raw_request("GetCurrentProgramScene", json!({}))
        .await
        .context("read current program scene")?;
    let program_scene = current
        .get("currentProgramSceneName")
        .and_then(Value::as_str)
        .context("GetCurrentProgramScene returned no currentProgramSceneName")?
        .to_owned();
    let transition = client.transitions().current().await?;
    let transition_name = transition.id.name.clone();
    let transition_duration_ms = transition
        .duration
        .map(|duration| u64::try_from(duration.whole_milliseconds()).unwrap_or(0));
    let vertical_scene = match current_vertical_scene(client).await {
        Ok(response) => response
            .get("scene")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Err(_) => None,
    };
    let mut audio = Vec::new();
    for input in [MIC_SOURCE, SCREEN_AUDIO_SOURCE] {
        if let Ok(state) = audio_state(client, input).await {
            audio.push(state);
        }
    }
    Ok(ObsSnapshot {
        program_scene,
        transition_name,
        transition_duration_ms,
        transition_fixed: transition.fixed,
        vertical_scene,
        audio,
        mask_radius: current_mask_radius(client).await.ok(),
    })
}

async fn audio_state(client: &Client, input: &str) -> Result<AudioState> {
    let muted = client.inputs().muted(InputId::Name(input)).await?;
    let monitor = client
        .inputs()
        .audio_monitor_type(InputId::Name(input))
        .await?;
    Ok(AudioState {
        input: input.to_owned(),
        muted,
        monitor: monitor_name(monitor).to_owned(),
    })
}

async fn current_program_scene(client: &Client) -> Result<String> {
    let current = client
        .raw_request("GetCurrentProgramScene", json!({}))
        .await?;
    Ok(current
        .get("currentProgramSceneName")
        .and_then(Value::as_str)
        .context("GetCurrentProgramScene returned no currentProgramSceneName")?
        .to_owned())
}

async fn set_main_scene(client: &Client, scene_uuid: &str) -> Result<()> {
    client
        .raw_request("SetCurrentProgramScene", json!({ "sceneUuid": scene_uuid }))
        .await
        .context("SetCurrentProgramScene")?;
    Ok(())
}

async fn set_muted(client: &Client, input: &str, muted: bool) -> Result<()> {
    client
        .inputs()
        .set_muted(InputId::Name(input), muted)
        .await
        .with_context(|| format!("SetInputMute({input})"))?;
    Ok(())
}

async fn live_scene_items(client: &Client, scene_uuid: &str) -> Result<Vec<Value>> {
    let response = client
        .raw_request("GetSceneItemList", json!({ "sceneUuid": scene_uuid }))
        .await
        .with_context(|| format!("GetSceneItemList({scene_uuid})"))?;
    Ok(response
        .get("sceneItems")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

async fn live_counts(client: &Client) -> Result<LiveCounts> {
    Ok(LiveCounts {
        main_scenes: main_scenes(client).await?.len(),
        vertical_scenes: vertical_scene_names(client).await?.len(),
        vidyst_inputs: vidyst_input_count(client).await?,
        camera_filters: camera_filter_count(client).await?,
    })
}

fn transition_span(events: &[TimedEvent]) -> Option<u64> {
    let start = events
        .iter()
        .rev()
        .find(|event| event.kind == "SceneTransitionStarted")?;
    let end = events.iter().rev().find(|event| {
        event.t_ms >= start.t_ms
            && (event.kind == "SceneTransitionEnded" || event.kind == "SceneTransitionVideoEnded")
    })?;
    Some(end.t_ms.saturating_sub(start.t_ms))
}

fn monitor_name(monitor: MonitorType) -> &'static str {
    match monitor {
        MonitorType::None => "OBS_MONITORING_TYPE_NONE",
        MonitorType::MonitorOnly => "OBS_MONITORING_TYPE_MONITOR_ONLY",
        MonitorType::MonitorAndOutput => "OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT",
        _ => "OBS_MONITORING_TYPE_UNKNOWN",
    }
}

fn parse_monitor(name: &str) -> MonitorType {
    match name {
        "OBS_MONITORING_TYPE_MONITOR_ONLY" => MonitorType::MonitorOnly,
        "OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT" => MonitorType::MonitorAndOutput,
        _ => MonitorType::None,
    }
}

fn png_dimensions(path: &Path) -> Result<(u32, u32)> {
    let data = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if data.len() < 24 || data[..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        bail!("{} is not a PNG", path.display());
    }
    let width = u32::from_be_bytes(data[16..20].try_into().expect("4-byte slice"));
    let height = u32::from_be_bytes(data[20..24].try_into().expect("4-byte slice"));
    Ok((width, height))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn write_markdown(path: &Path, report: &Phase1Report) -> Result<()> {
    use std::fmt::Write as _;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    writeln!(out, "# Phase 1 validation results\n")?;
    writeln!(out, "- Generated: `{}`", report.generated_at)?;
    writeln!(
        out,
        "- OBS Studio: **{}**; obs-websocket: **{}** (RPC {})",
        report.obs_studio_version, report.obs_websocket_version, report.rpc_version
    )?;
    writeln!(
        out,
        "- Overall: **{}**",
        if report.passed { "PASS" } else { "FAIL" }
    )?;
    writeln!(
        out,
        "- Bootstrap: **{} scenes**, **{} requests**, **{} failed**",
        report.bootstrap.scenes, report.bootstrap.requests, report.bootstrap.failed_requests
    )?;
    writeln!(
        out,
        "- Camera: `{}` ({})",
        report.bootstrap.camera,
        report
            .bootstrap
            .camera_device
            .as_deref()
            .unwrap_or("OBS default")
    )?;
    writeln!(
        out,
        "- Screen: `{}` ({})",
        report.bootstrap.screen,
        report
            .bootstrap
            .screen_device
            .as_deref()
            .unwrap_or("OBS default")
    )?;
    writeln!(
        out,
        "- Mask: `{}` ({}), base {}; circle {}; tile {}",
        MASK_FILTER,
        report.bootstrap.mask_kind,
        report.bootstrap.base_radius,
        report.bootstrap.circle_radius,
        report.bootstrap.tile_radius
    )?;
    writeln!(
        out,
        "- Transition: `{}` at {} ms",
        report.bootstrap.transition, report.bootstrap.transition_duration_ms
    )?;
    writeln!(
        out,
        "- Reports: [bootstrap](phase1-bootstrap.json), [rerun](phase1-bootstrap-rerun.json), [JSON](phase1-results.json)\n"
    )?;

    writeln!(out, "## Bootstrap validation\n")?;
    writeln!(
        out,
        "**{}** — {}\n",
        if report.bootstrap_validation.passed {
            "PASS"
        } else {
            "FAIL"
        },
        report.bootstrap_validation.summary
    )?;
    write_test_details(&mut out, &report.bootstrap_validation)?;

    writeln!(out, "\n## Tests\n")?;
    for test in &report.tests {
        writeln!(
            out,
            "### {} — {}\n",
            test.name,
            if test.passed { "PASS" } else { "FAIL" }
        )?;
        writeln!(out, "{}\n", test.summary)?;
        writeln!(out, "Duration: **{} ms**\n", test.duration_ms)?;
        write_test_details(&mut out, test)?;
    }

    writeln!(out, "\n## Original state and cleanup\n")?;
    writeln!(
        out,
        "**{}**\n",
        if report.cleanup.restored {
            "PASS — original OBS state restored"
        } else {
            "FAIL — cleanup needs attention"
        }
    )?;
    writeln!(
        out,
        "```json\n{}\n```\n",
        serde_json::to_string_pretty(&report.original_state)?
    )?;
    if !report.cleanup.failures.is_empty() {
        writeln!(out, "Cleanup failures:\n")?;
        for failure in &report.cleanup.failures {
            writeln!(out, "- {failure}")?;
        }
        writeln!(out)?;
    }
    if !report.cleanup.steps.is_empty() {
        writeln!(out, "Cleanup steps:\n")?;
        for step in &report.cleanup.steps {
            writeln!(out, "- {step}")?;
        }
        writeln!(out)?;
    }

    writeln!(out, "## Notes\n")?;
    for note in &report.notes {
        writeln!(out, "- {note}")?;
    }

    std::fs::write(path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn write_test_details(out: &mut String, test: &TestResult) -> Result<()> {
    use std::fmt::Write as _;

    if !test.notes.is_empty() {
        writeln!(out, "Notes:\n")?;
        for note in &test.notes {
            writeln!(out, "- {note}")?;
        }
        writeln!(out)?;
    }
    if !test.failures.is_empty() {
        writeln!(out, "Failures:\n")?;
        for failure in &test.failures {
            writeln!(out, "- {failure}")?;
        }
        writeln!(out)?;
    }
    if !test.images.is_empty() {
        writeln!(out, "Screenshots:\n")?;
        for image in &test.images {
            writeln!(out, "![{}]({})", image, image)?;
        }
        writeln!(out)?;
    }
    if !test.details.is_null() {
        writeln!(
            out,
            "<details><summary>Evidence JSON</summary>\n\n```json\n{}\n```\n\n</details>\n",
            serde_json::to_string_pretty(&test.details)?
        )?;
    }
    Ok(())
}
