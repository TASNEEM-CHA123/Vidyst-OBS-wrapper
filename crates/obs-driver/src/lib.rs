//! OBS WebSocket driver for Vidyst-OBS.
//!
//! Phase 0 currently exposes environment discovery and plugin verification.
//! Scene bootstrapping and layout switching are added in Phase 1.

pub mod bootstrap;
pub mod discovery;
pub mod phase1;

pub use bootstrap::{run_bootstrap, BootstrapOptions, BootstrapReport, RequestLog};
pub use discovery::{
    detect_installation, load_websocket_password, ObsInstallation, PluginStatus, WsCredentials,
};
pub use phase1::{run_phase1, ConnectionInfo, Phase1Options, Phase1Report};
