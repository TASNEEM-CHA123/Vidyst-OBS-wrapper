//! Dev-profile and clean-state helper for Vidyst-OBS.
//!
//! Keeps test runs out of the user's real OBS scene collection and restores a
//! predictable clean state afterwards. All mutations go through obs-websocket;
//! scene-collection JSON is never hand-edited.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use obws::{
    common::MonitorType,
    requests::{inputs::InputId, scenes::SceneId},
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vidyst_obs_driver::{
    bootstrap::{
        current_vertical_scene, run_bootstrap, switch_vertical_scene, BootstrapOptions, CAMERA_RAW,
        MIC_SOURCE, SCREEN_AUDIO_SOURCE, SCREEN_SOURCE,
    },
    load_websocket_password,
};

const BACKGROUND_SOURCE: &str = "Vidyst_Background";
const BACKGROUND_IMAGE_SOURCE: &str = "Vidyst_Background_Image";
const WINDOW_SOURCE: &str = "Vidyst_Window";
const BACKGROUND_FILTER: &str = "Vidyst_Background_Removal";
const DEV_COLLECTION: &str = "Vidyst_Dev";
const DEFAULT_STATE: &str = ".dev-state/original.json";

#[derive(Debug)]
struct Args {
    command: String,
    host: String,
    port: u16,
    password: Option<String>,
    password_env: Option<String>,
    state: PathBuf,
    collection: Option<String>,
    scene: Option<String>,
    transition: Option<String>,
    duration_ms: u64,
    vertical_scene: Option<String>,
    request_type: Option<String>,
    request_data: Value,
    force_save: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            command: String::new(),
            host: "127.0.0.1".to_owned(),
            port: 4455,
            password: None,
            password_env: None,
            state: PathBuf::from(DEFAULT_STATE),
            collection: None,
            scene: None,
            transition: None,
            duration_ms: 300,
            vertical_scene: None,
            request_type: None,
            request_data: json!({}),
            force_save: false,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct SavedState {
    scene_collection: Option<String>,
    program_scene: Option<String>,
    transition: Option<String>,
    transition_duration_ms: Option<u64>,
    vertical_scene: Option<String>,
    audio: Vec<SavedAudio>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SavedAudio {
    input: String,
    muted: bool,
    monitor_type: String,
    volume_db: f64,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args()?;
    if args.command.is_empty() {
        bail!("missing command; use devctl --help");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create Tokio runtime")?;

    runtime.block_on(async move {
        let explicit_password = match (&args.password, &args.password_env) {
            (Some(_), Some(_)) => bail!("use either --password or --password-env, not both"),
            (Some(password), None) => Some(password.clone()),
            (None, Some(variable)) => Some(
                std::env::var(variable)
                    .with_context(|| format!("read password from {variable}"))?,
            ),
            (None, None) => None,
        };
        let (password, source) = load_websocket_password(explicit_password.as_deref());
        println!("credentials: {source:?}");
        let client = Client::connect(&args.host, args.port, password.as_deref())
            .await
            .context("connect to OBS WebSocket")?;

        match args.command.as_str() {
            "save-original" => {
                let state = capture_state(&client).await?;
                write_state(&args.state, &state)?;
                println!("saved original state to {}", args.state.display());
                print_state(&state);
            }
            "setup-dev" => setup_dev(&client, &args).await?,
            "reset" => {
                reset_active(&client, &args).await?;
            }
            "restore" => restore(&client, &args).await?,
            "status" => {
                let state = capture_state(&client).await?;
                print_state(&state);
                let scenes = client.scenes().list().await?.scenes;
                println!("main scenes: {}", scenes.len());
            }
            "probe" => probe(&client, &args).await?,
            other => bail!("unknown command {other}; use devctl --help"),
        }
        Ok(())
    })
}

async fn setup_dev(client: &Client, args: &Args) -> Result<()> {
    let original = capture_state(client).await?;
    if args.force_save || !args.state.exists() {
        write_state(&args.state, &original)?;
        println!("saved original state to {}", args.state.display());
        print_state(&original);
    } else {
        println!(
            "keeping existing original state at {}",
            args.state.display()
        );
    }

    let collection = args.collection.as_deref().unwrap_or(DEV_COLLECTION);
    let collections = client.scene_collections().list().await?;
    if collections
        .collections
        .iter()
        .any(|name| name == collection)
    {
        client
            .scene_collections()
            .set_current(collection)
            .await
            .with_context(|| format!("switch to scene collection {collection}"))?;
    } else {
        client
            .scene_collections()
            .create(collection)
            .await
            .with_context(|| format!("create scene collection {collection}"))?;
    }
    println!("active scene collection: {collection}");

    let report = run_bootstrap(
        client,
        &BootstrapOptions {
            camera_connected: true,
            screen_connected: true,
            ..BootstrapOptions::default()
        },
    )
    .await?;
    let failed = report.requests.iter().filter(|request| !request.ok).count();
    println!(
        "bootstrap: {} scenes, {} requests, {failed} failed",
        report.scenes.len(),
        report.requests.len()
    );
    for request in report.requests.iter().filter(|request| !request.ok) {
        println!(
            "  FAILED {}: {}",
            request.label,
            request.error.as_deref().unwrap_or("unknown")
        );
    }
    Ok(())
}

async fn restore(client: &Client, args: &Args) -> Result<()> {
    let state = read_state(&args.state)?;
    let collection = args
        .collection
        .clone()
        .or_else(|| state.scene_collection.clone())
        .context("no original scene collection saved; use --collection")?;
    let collections = client.scene_collections().list().await?;
    if !collections
        .collections
        .iter()
        .any(|name| name == &collection)
    {
        bail!("original scene collection {collection} no longer exists");
    }
    client
        .scene_collections()
        .set_current(&collection)
        .await
        .with_context(|| format!("switch back to scene collection {collection}"))?;
    println!("restored scene collection: {collection}");

    let restore_args = Args {
        scene: args.scene.clone().or_else(|| Some("Scene".to_owned())),
        transition: args.transition.clone().or_else(|| Some("Fade".to_owned())),
        vertical_scene: args
            .vertical_scene
            .clone()
            .or_else(|| Some("Vertical Scene".to_owned())),
        ..args_clone(args)
    };
    reset_active(client, &restore_args).await?;
    restore_audio(client, &state.audio).await?;
    Ok(())
}

fn args_clone(args: &Args) -> Args {
    Args {
        command: args.command.clone(),
        host: args.host.clone(),
        port: args.port,
        password: args.password.clone(),
        password_env: args.password_env.clone(),
        state: args.state.clone(),
        collection: args.collection.clone(),
        scene: args.scene.clone(),
        transition: args.transition.clone(),
        duration_ms: args.duration_ms,
        vertical_scene: args.vertical_scene.clone(),
        request_type: args.request_type.clone(),
        request_data: args.request_data.clone(),
        force_save: args.force_save,
    }
}

async fn reset_active(client: &Client, args: &Args) -> Result<()> {
    // Program scene.
    let scenes = client.scenes().list().await?.scenes;
    let scene_names: Vec<String> = scenes.iter().map(|scene| scene.id.name.clone()).collect();
    if let Some(scene) = args
        .scene
        .as_deref()
        .filter(|scene| scene_names.iter().any(|name| name == scene))
    {
        client
            .scenes()
            .set_current_program_scene(SceneId::Name(scene))
            .await
            .with_context(|| format!("set program scene {scene}"))?;
        println!("program scene: {scene}");
    } else {
        println!("program scene: kept (requested scene not present)");
    }

    // Transition.
    let transition_name = args.transition.as_deref().unwrap_or("Fade");
    let transitions = client.transitions().list().await?;
    if transitions
        .transitions
        .iter()
        .any(|transition| transition.id.name == transition_name)
    {
        client.transitions().set_current(transition_name).await?;
        client
            .transitions()
            .set_current_duration(time::Duration::milliseconds(args.duration_ms as i64))
            .await?;
        println!("transition: {transition_name} {} ms", args.duration_ms);
    } else {
        println!("transition: kept ({transition_name} not present)");
    }

    // Vertical scene.
    let vertical_name = args.vertical_scene.as_deref().unwrap_or("Vertical Scene");
    let vertical_scenes = vidyst_obs_driver::bootstrap::vertical_scene_names(client)
        .await
        .unwrap_or_default();
    if vertical_scenes.iter().any(|name| name == vertical_name) {
        switch_vertical_scene(client, vertical_name).await?;
        println!("vertical scene: {vertical_name}");
    } else {
        println!("vertical scene: kept ({vertical_name} not present)");
    }

    // Background and window capture off.
    for source in [BACKGROUND_SOURCE, BACKGROUND_IMAGE_SOURCE, WINDOW_SOURCE] {
        hide_source_everywhere(client, source).await?;
    }
    println!("background/window sources hidden");

    // Background removal off.
    let filters = client
        .filters()
        .list(obws::requests::sources::SourceId::Name(CAMERA_RAW))
        .await
        .unwrap_or_default();
    if filters
        .iter()
        .any(|filter| filter.name == BACKGROUND_FILTER)
    {
        client
            .raw_request(
                "SetSourceFilterEnabled",
                json!({
                    "sourceName": CAMERA_RAW,
                    "filterName": BACKGROUND_FILTER,
                    "filterEnabled": false,
                }),
            )
            .await
            .ok();
        println!("background removal: off");
    }

    // Display capture on the first available monitor.
    if let Some(value) = first_property_value(client, SCREEN_SOURCE, "monitor_id").await {
        set_input_settings(client, SCREEN_SOURCE, json!({ "monitor_id": value })).await?;
        println!("screen source: display capture");
    }

    // Camera reconnected to the first available device.
    if let Some(value) = first_property_value(client, CAMERA_RAW, "video_device_id").await {
        set_input_settings(
            client,
            CAMERA_RAW,
            json!({
                "video_device_id": value,
                "resolution": "1280x720",
                "frame_interval": -1,
                "video_format": 0,
            }),
        )
        .await?;
        println!("camera: reconnected");
    }

    // Safe audio defaults.
    for input in [MIC_SOURCE, SCREEN_AUDIO_SOURCE] {
        if input_exists(client, input).await? {
            client
                .inputs()
                .set_muted(InputId::Name(input), false)
                .await
                .ok();
            client
                .inputs()
                .set_audio_monitor_type(InputId::Name(input), MonitorType::None)
                .await
                .ok();
            println!("audio {input}: unmuted, monitoring off");
        }
    }

    Ok(())
}

async fn restore_audio(client: &Client, audio: &[SavedAudio]) -> Result<()> {
    for saved in audio {
        if !input_exists(client, &saved.input).await? {
            continue;
        }
        client
            .inputs()
            .set_muted(InputId::Name(&saved.input), saved.muted)
            .await
            .ok();
        let monitor = match saved.monitor_type.as_str() {
            "OBS_MONITORING_TYPE_MONITOR_ONLY" => MonitorType::MonitorOnly,
            "OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT" => MonitorType::MonitorAndOutput,
            _ => MonitorType::None,
        };
        client
            .inputs()
            .set_audio_monitor_type(InputId::Name(&saved.input), monitor)
            .await
            .ok();
        client
            .inputs()
            .set_volume(
                InputId::Name(&saved.input),
                obws::requests::inputs::Volume::Db(saved.volume_db as f32),
            )
            .await
            .ok();
        println!(
            "audio {}: muted={} monitor={} volume={:.1} dB",
            saved.input, saved.muted, saved.monitor_type, saved.volume_db
        );
    }
    Ok(())
}

async fn hide_source_everywhere(client: &Client, source: &str) -> Result<()> {
    let scenes = client.scenes().list().await?.scenes;
    for scene in scenes {
        let scene_name = &scene.id.name;
        let items = client
            .scene_items()
            .list(SceneId::Name(scene_name))
            .await
            .unwrap_or_default();
        for item in items {
            if item.source_name == source {
                client
                    .raw_request(
                        "SetSceneItemEnabled",
                        json!({
                            "sceneName": scene_name,
                            "sceneItemId": item.id,
                            "sceneItemEnabled": false,
                        }),
                    )
                    .await
                    .ok();
            }
        }
    }
    Ok(())
}

async fn capture_state(client: &Client) -> Result<SavedState> {
    let scene_collection = client.scene_collections().current().await.ok();
    let program_scene = client
        .scenes()
        .current_program_scene()
        .await
        .ok()
        .map(|scene| scene.id.name);
    let transition = client.transitions().current().await.ok();
    let transition_duration_ms = transition
        .as_ref()
        .and_then(|value| value.duration)
        .map(|duration| duration.whole_milliseconds().max(0) as u64);
    let transition = transition.map(|value| value.id.name);
    let vertical_scene = current_vertical_scene(client).await.ok().and_then(|value| {
        value
            .get("scene")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });

    let mut audio = Vec::new();
    for input in [MIC_SOURCE, SCREEN_AUDIO_SOURCE] {
        if !input_exists(client, input).await.unwrap_or(false) {
            continue;
        }
        let muted = client
            .inputs()
            .muted(InputId::Name(input))
            .await
            .unwrap_or(false);
        let monitor_type = client
            .inputs()
            .audio_monitor_type(InputId::Name(input))
            .await
            .ok()
            .and_then(|value| serde_json::to_value(value).ok())
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "OBS_MONITORING_TYPE_NONE".to_owned());
        let volume_db = client
            .inputs()
            .volume(InputId::Name(input))
            .await
            .map(|value| value.db)
            .unwrap_or(0.0);
        audio.push(SavedAudio {
            input: input.to_owned(),
            muted,
            monitor_type,
            volume_db: f64::from(volume_db),
        });
    }

    Ok(SavedState {
        scene_collection,
        program_scene,
        transition,
        transition_duration_ms,
        vertical_scene,
        audio,
    })
}

fn write_state(path: &Path, state: &SavedState) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
    }
    fs::write(path, serde_json::to_string_pretty(state)?)
        .with_context(|| format!("write {}", path.display()))
}

fn read_state(path: &Path) -> Result<SavedState> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn print_state(state: &SavedState) {
    println!(
        "scene collection : {}",
        state.scene_collection.as_deref().unwrap_or("?")
    );
    println!(
        "program scene    : {}",
        state.program_scene.as_deref().unwrap_or("?")
    );
    println!(
        "transition       : {} ({} ms)",
        state.transition.as_deref().unwrap_or("?"),
        state.transition_duration_ms.unwrap_or(0)
    );
    println!(
        "vertical scene   : {}",
        state.vertical_scene.as_deref().unwrap_or("?")
    );
    for audio in &state.audio {
        println!(
            "audio {:16}: muted={} monitor={} volume={:.1} dB",
            audio.input, audio.muted, audio.monitor_type, audio.volume_db
        );
    }
}

async fn probe(client: &Client, args: &Args) -> Result<()> {
    let request_type = args
        .request_type
        .as_deref()
        .context("probe requires --request-type")?;
    let response: obws::responses::general::VendorResponse<Value> = client
        .general()
        .call_vendor_request(obws::requests::general::CallVendorRequest {
            vendor_name: "aitum-vertical-canvas",
            request_type,
            request_data: &args.request_data,
        })
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&response.response_data).unwrap_or_default()
    );
    Ok(())
}

async fn input_exists(client: &Client, name: &str) -> Result<bool> {
    Ok(client
        .inputs()
        .list(None)
        .await?
        .iter()
        .any(|input| input.id.name == name))
}

async fn first_property_value(client: &Client, input: &str, property: &str) -> Option<String> {
    let response = client
        .raw_request(
            "GetInputPropertiesListPropertyItems",
            json!({ "inputName": input, "propertyName": property }),
        )
        .await
        .ok()?;
    response
        .get("propertyItems")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("itemValue"))
        .cloned()
        .map(|value| match value {
            Value::String(value) => value,
            other => other.to_string(),
        })
}

async fn set_input_settings(client: &Client, input: &str, settings: Value) -> Result<()> {
    client
        .raw_request(
            "SetInputSettings",
            json!({
                "inputName": input,
                "inputSettings": settings,
                "overlay": true,
            }),
        )
        .await
        .with_context(|| format!("update input settings for {input}"))?;
    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut args = Args::default();
    let mut raw = std::env::args().skip(1);

    while let Some(arg) = raw.next() {
        let (key, inline_value) = match arg.split_once('=') {
            Some((key, value)) => (key.to_owned(), Some(value.to_owned())),
            None => (arg.clone(), None),
        };
        let mut value = |name: &str| -> Result<String> {
            inline_value
                .clone()
                .or_else(|| raw.next())
                .with_context(|| format!("{name} requires a value"))
        };

        match key.as_str() {
            "--help" | "-h" => {
                println!(
                    "devctl <save-original|setup-dev|reset|restore|status|probe> [options]\n\n\
                     Options:\n\
                     \x20 --host HOST            default 127.0.0.1\n\
                     \x20 --port PORT            default 4455\n\
                     \x20 --password PW          or --password-env VAR\n\
                     \x20 --state PATH           default {DEFAULT_STATE}\n\
                     \x20 --collection NAME      scene collection to activate/restore\n\
                     \x20 --force-save           overwrite the saved original state\n\
                     reset options:\n\
                     \x20 --scene NAME           default Scene (kept if missing)\n\
                     \x20 --transition NAME      default Fade\n\
                     \x20 --duration-ms N        default 300\n\
                     \x20 --vertical-scene NAME  default Vertical Scene\n\
                     probe options:\n\
                     \x20 --request-type NAME    Aitum vendor request\n\
                     \x20 --data JSON            request payload (default {{}})\n\n\
                     setup-dev creates/switches to Vidyst_Dev and runs bootstrap.\n\
                     restore switches back to the saved collection and resets it."
                );
                std::process::exit(0);
            }
            "--host" => args.host = value("--host")?,
            "--port" => args.port = value("--port")?.parse().context("parse --port")?,
            "--password" => args.password = Some(value("--password")?),
            "--password-env" => args.password_env = Some(value("--password-env")?),
            "--state" => args.state = PathBuf::from(value("--state")?),
            "--collection" => args.collection = Some(value("--collection")?),
            "--scene" => args.scene = Some(value("--scene")?),
            "--transition" => args.transition = Some(value("--transition")?),
            "--duration-ms" => {
                args.duration_ms = value("--duration-ms")?
                    .parse()
                    .context("parse --duration-ms")?
            }
            "--vertical-scene" => args.vertical_scene = Some(value("--vertical-scene")?),
            "--request-type" => args.request_type = Some(value("--request-type")?),
            "--data" => {
                args.request_data =
                    serde_json::from_str(&value("--data")?).context("parse --data JSON")?
            }
            "--force-save" => args.force_save = true,
            other if other.starts_with('-') => bail!("unknown argument {other}; use --help"),
            other => {
                if args.command.is_empty() {
                    args.command = other.to_owned();
                } else {
                    bail!("unexpected argument {other}; use --help");
                }
            }
        }
    }

    Ok(args)
}
