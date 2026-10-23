//! Phase 1 bootstrap + validation spike.
//!
//! Runs the idempotent OBS bootstrap, validates the live scene graph, executes
//! the five Phase 1 tests, restores the original OBS state, and writes the
//! generated reports under `docs/`.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use vidyst_obs_driver::bootstrap::{run_bootstrap, BootstrapOptions, BootstrapReport};
use vidyst_obs_driver::load_websocket_password;
use vidyst_obs_driver::phase1::{run_phase1, ConnectionInfo, Phase1Options};

#[derive(Debug)]
struct Args {
    host: String,
    port: u16,
    password: Option<String>,
    password_env: Option<String>,
    report: Option<PathBuf>,
    results: Option<PathBuf>,
    markdown: Option<PathBuf>,
    screenshots: Option<PathBuf>,
    bootstrap_only: bool,
    camera: Option<String>,
    monitor: Option<String>,
    mic: Option<String>,
    screen_audio: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 4455,
            password: None,
            password_env: None,
            report: Some(PathBuf::from("docs/phase1-bootstrap.json")),
            results: None,
            markdown: None,
            screenshots: None,
            bootstrap_only: false,
            camera: None,
            monitor: None,
            mic: None,
            screen_audio: None,
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args()?;
    if args.port == 0 {
        bail!("--port must be between 1 and 65535");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create Tokio runtime")?;

    runtime.block_on(async move {
        let explicit_password =
            match (&args.password, &args.password_env) {
                (Some(_), Some(_)) => bail!("use either --password or --password-env, not both"),
                (Some(password), None) => Some(password.clone()),
                (None, Some(variable)) => Some(std::env::var(variable).with_context(|| {
                    format!("read password from environment variable {variable}")
                })?),
                (None, None) => None,
            };
        let (password, source) = load_websocket_password(explicit_password.as_deref());
        println!("credentials: {source:?}");
        println!("connecting to {}:{}", args.host, args.port);

        let client = obws::Client::connect(&args.host, args.port, password.as_deref())
            .await
            .context("connect to OBS WebSocket")?;

        let bootstrap_options = BootstrapOptions {
            camera_device: args.camera.clone(),
            monitor: args.monitor.clone(),
            mic_device: args.mic.clone(),
            screen_audio_device: args.screen_audio.clone(),
            camera_connected: true,
            screen_connected: true,
        };

        if args.bootstrap_only {
            let report = run_bootstrap(&client, &bootstrap_options).await?;
            print_bootstrap(&report);
            if let Some(path) = &args.report {
                write_json(path, &report)?;
                println!("report: {}", path.display());
            }
            return if report.requests.iter().all(|request| request.ok) {
                Ok(())
            } else {
                bail!("bootstrap completed with failed requests")
            };
        }

        let mut phase_options = Phase1Options::default();
        if let Some(path) = args.report {
            phase_options.bootstrap_report = path;
        }
        phase_options.bootstrap_rerun_report = phase_options
            .bootstrap_report
            .with_file_name("phase1-bootstrap-rerun.json");
        if let Some(path) = args.results {
            phase_options.results_json = path;
        }
        if let Some(path) = args.markdown {
            phase_options.results_markdown = path;
        }
        if let Some(path) = args.screenshots {
            phase_options.screenshot_dir = path;
        }
        if let Some(parent) = phase_options.results_markdown.parent() {
            if !parent.as_os_str().is_empty() {
                phase_options.docs_dir = parent.to_path_buf();
            }
        }

        let connection = ConnectionInfo {
            host: args.host,
            port: args.port,
            password,
        };
        let report = run_phase1(client, &connection, &bootstrap_options, &phase_options).await?;

        println!("Phase 1: {}", if report.passed { "PASS" } else { "FAIL" });
        println!(
            "bootstrap: {} scenes, {} requests ({} failed)",
            report.bootstrap.scenes, report.bootstrap.requests, report.bootstrap.failed_requests
        );
        for test in &report.tests {
            println!(
                "{} {} ({} ms): {}",
                if test.passed { "PASS" } else { "FAIL" },
                test.id,
                test.duration_ms,
                test.summary
            );
            for failure in &test.failures {
                eprintln!("  {}", failure);
            }
        }
        println!(
            "cleanup restored original state: {}",
            report.cleanup.restored
        );
        for failure in &report.cleanup.failures {
            eprintln!("  cleanup: {}", failure);
        }
        println!("results JSON: {}", phase_options.results_json.display());
        println!(
            "results Markdown: {}",
            phase_options.results_markdown.display()
        );

        if report.passed {
            Ok(())
        } else {
            bail!("Phase 1 validation failed")
        }
    })
}

fn print_bootstrap(report: &BootstrapReport) {
    let failed: Vec<_> = report
        .requests
        .iter()
        .filter(|request| !request.ok)
        .collect();
    println!(
        "bootstrap ok: {} scenes, {} requests ({} failed)",
        report.scenes.len(),
        report.requests.len(),
        failed.len()
    );
    println!(
        "camera: {} [{}]",
        report.camera.name,
        report
            .camera
            .device_name
            .as_deref()
            .unwrap_or("OBS default")
    );
    println!(
        "screen: {} [{}]",
        report.screen.name,
        report
            .screen
            .device_name
            .as_deref()
            .unwrap_or("OBS default")
    );
    println!(
        "mask: {} radius {} (circle {} / tile {})",
        report.filters.mask_filter,
        report.filters.base_radius,
        report.filters.circle_target_radius,
        report.filters.tile_target_radius
    );
    println!(
        "transition: {} {}ms",
        report.transition.name, report.transition.duration_ms
    );
    for request in failed {
        eprintln!(
            "FAILED request: {}: {}",
            request.label,
            request.error.as_deref().unwrap_or("unknown error")
        );
    }
}

fn write_json<T: serde::Serialize>(path: &std::path::Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("write {}", path.display()))?;
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
                    "spike [--host HOST] [--port PORT] [--password PW | --password-env VAR]\n\
                     \x20     [--camera MATCH] [--monitor MATCH] [--mic MATCH] [--screen-audio MATCH]\n\
                     \x20     [--report PATH] [--results PATH] [--markdown PATH] [--screenshots DIR]\n\
                     \x20     [--bootstrap-only] [--no-report]\n\n\
                     By default runs bootstrap + all five Phase 1 tests and restores the\n\
                     original scene, transition, and audio state. --bootstrap-only skips tests.\n\
                     The OBS WebSocket password is never logged. By default it is read from\n\
                     OBS_WS_PASSWORD or the obs-websocket config file."
                );
                std::process::exit(0);
            }
            "--host" => args.host = value("--host")?,
            "--port" => args.port = value("--port")?.parse().context("parse --port")?,
            "--password" => args.password = Some(value("--password")?),
            "--password-env" => args.password_env = Some(value("--password-env")?),
            "--report" => args.report = Some(PathBuf::from(value("--report")?)),
            "--no-report" => args.report = None,
            "--results" => args.results = Some(PathBuf::from(value("--results")?)),
            "--markdown" => args.markdown = Some(PathBuf::from(value("--markdown")?)),
            "--screenshots" => args.screenshots = Some(PathBuf::from(value("--screenshots")?)),
            "--bootstrap-only" => args.bootstrap_only = true,
            "--camera" => args.camera = Some(value("--camera")?),
            "--monitor" => args.monitor = Some(value("--monitor")?),
            "--mic" => args.mic = Some(value("--mic")?),
            "--screen-audio" => args.screen_audio = Some(value("--screen-audio")?),
            other => bail!("unknown argument {other}; use --help"),
        }
    }

    Ok(args)
}
