//! Phase 0 discovery CLI.
//!
//! Usage:
//!   cargo run -p vidyst-obs-driver --bin phase0
//!   cargo run -p vidyst-obs-driver --bin phase0 -- --host 127.0.0.1 --port 4455
//!   OBS_WS_PASSWORD=... cargo run -p vidyst-obs-driver --bin phase0 -- --json

use std::env;
use std::process::ExitCode;

use vidyst_obs_driver::discovery::{
    discover, install_help, load_websocket_password, WsCredentials,
};

struct Args {
    host: String,
    port: u16,
    password: Option<String>,
    json: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut host = "127.0.0.1".to_owned();
    let mut port = 4455u16;
    let mut password = None;
    let mut json = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => {
                host = args
                    .next()
                    .ok_or_else(|| "--host requires a value".to_owned())?
            }
            "--port" => {
                port = args
                    .next()
                    .ok_or_else(|| "--port requires a value".to_owned())?
                    .parse()
                    .map_err(|_| "--port must be a number".to_owned())?
            }
            "--password" => {
                password = Some(
                    args.next()
                        .ok_or_else(|| "--password requires a value".to_owned())?,
                )
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!(
                    "Vidyst-OBS Phase 0 discovery\n\n\
                     Options:\n\
                       --host <host>       OBS WebSocket host (default 127.0.0.1)\n\
                       --port <port>       OBS WebSocket port (default 4455)\n\
                       --password <pass>   Password (otherwise OBS_WS_PASSWORD or OBS config)\n\
                       --json              Print the full report as JSON\n\
                       --help              Show this help"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        host,
        port,
        password,
        json,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .try_init()
    {
        eprintln!("warning: failed to initialise logging: {err}");
    }

    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    let (password, credentials) = load_websocket_password(args.password.as_deref());

    match discover(&args.host, args.port, password.as_deref(), credentials).await {
        Ok(report) => {
            if args.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(err) => {
                        eprintln!("error: failed to serialise report: {err}");
                        return ExitCode::from(1);
                    }
                }
            } else {
                print_report(&args, &report);
            }

            if report.ready {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(err) => {
            eprintln!("Phase 0 discovery failed: {err:#}");
            eprintln!();
            eprintln!(
                "Open OBS Studio, then enable Tools -> WebSocket Server Settings -> Enable WebSocket Server."
            );
            eprintln!(
                "Use --password, OBS_WS_PASSWORD, or the local OBS config for authentication."
            );
            ExitCode::from(1)
        }
    }
}

fn print_report(args: &Args, report: &vidyst_obs_driver::discovery::DiscoveryReport) {
    println!("Vidyst-OBS Phase 0 discovery");
    println!("============================");
    println!("OS:                 {}", report.installation.os);
    println!(
        "OBS binary:         {}",
        report
            .installation
            .binary
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "not found".to_owned())
    );
    println!(
        "WebSocket:          {}:{} ({})",
        args.host,
        args.port,
        if report.websocket_reachable {
            "reachable"
        } else {
            "unreachable"
        }
    );
    println!(
        "Credentials:        {}",
        credential_label(report.websocket_credentials)
    );
    println!();
    println!("OBS Studio:         {}", report.runtime.obs_studio_version);
    println!(
        "obs-websocket:      {}",
        report.runtime.obs_websocket_version
    );
    println!("RPC version:        {}", report.runtime.rpc_version);
    println!(
        "Platform:           {}",
        report.runtime.platform_description
    );
    println!(
        "Kinds:              {} input, {} filter, {} transition",
        report.runtime.input_kind_count,
        report.runtime.filter_kind_count,
        report.runtime.transition_kind_count
    );

    let interesting_filters: Vec<_> = report
        .runtime
        .filter_kinds
        .iter()
        .filter(|kind| {
            kind.contains("move")
                || kind.contains("mask")
                || kind.contains("background")
                || kind.contains("multi")
        })
        .map(String::as_str)
        .collect();
    if !interesting_filters.is_empty() {
        println!("Relevant filters:   {}", interesting_filters.join(", "));
    }

    let interesting_transitions: Vec<_> = report
        .runtime
        .transition_kinds
        .iter()
        .filter(|kind| kind.contains("move") || kind.contains("fade") || kind.contains("cut"))
        .map(String::as_str)
        .collect();
    if !interesting_transitions.is_empty() {
        println!(
            "Relevant transitions: {}",
            interesting_transitions.join(", ")
        );
    }

    println!(
        "Transition instances: {}",
        if report.runtime.transition_instances.is_empty() {
            "(none)".to_owned()
        } else {
            report.runtime.transition_instances.join(", ")
        }
    );
    println!(
        "Move transition:     {}",
        report
            .runtime
            .move_transition_instance
            .as_deref()
            .unwrap_or("not created")
    );

    println!();
    println!("Canvases:");
    for canvas in &report.runtime.canvas_list {
        println!(
            "  - {}{} ({}x{})",
            canvas.name,
            if canvas.is_main { " [main]" } else { "" },
            canvas.width,
            canvas.height
        );
    }

    println!();
    println!("Required plugins:");
    for plugin in &report.plugins {
        println!(
            "  [{}] {}{}",
            if plugin.installed { "OK" } else { "MISSING" },
            plugin.name,
            plugin
                .note
                .as_ref()
                .map(|note| format!(" — {note}"))
                .unwrap_or_default()
        );
        for evidence in &plugin.evidence {
            println!("        evidence: {evidence}");
        }
    }

    if !report.warnings.is_empty() {
        println!();
        println!("Warnings:");
        for warning in &report.warnings {
            println!("  ! {warning}");
        }
    }

    if report.ready {
        println!();
        println!("Phase 0 gate: PASS — OBS and all required plugins are available.");
    } else {
        println!();
        println!(
            "Phase 0 gate: BLOCKED — missing: {}",
            report.missing_plugins.join(", ")
        );
        println!();
        println!("{}", install_help(&report.installation));
        println!();
        println!("Download pages:");
        for plugin in report.plugins.iter().filter(|plugin| !plugin.installed) {
            println!("  - {}: {}", plugin.name, plugin.install_url);
        }
        println!();
        println!(
            "After installing, restart OBS and run this command again. Use --json for a full report."
        );
    }
}

fn credential_label(source: WsCredentials) -> &'static str {
    match source {
        WsCredentials::Explicit => "--password",
        WsCredentials::Environment => "OBS_WS_PASSWORD",
        WsCredentials::ObsConfig => "OBS websocket config",
        WsCredentials::None => "none (no auth)",
    }
}
