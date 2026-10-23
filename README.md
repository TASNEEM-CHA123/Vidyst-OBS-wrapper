# Vidyst-OBS

A lightweight companion studio controller for OBS Studio. OBS remains the capture, compositing, and encoding engine; this app drives OBS through `obs-websocket` v5.

> **Current status:** Phase 0 and Phase 1 are complete. Phase 2 is in progress: the Tauri/Svelte studio UI, live embedded previews, audio meters, aspect-aware recording, and camera/screen connection controls are working. Live streaming validation is pending an Aitum vertical destination and RTMP targets.

## Requirements

- Windows 10/11 (current development machine: Windows 11 26H2)
- OBS Studio **31.1+** (verified on **32.2.2**)
- OBS built-in `obs-websocket` v5 enabled on port **4455**
- Rust **1.89+** with the MSVC toolchain
- Node.js 20+ (for the Phase 2 UI)

## Required OBS plugins

Install these into `C:\ProgramData\obs-studio\plugins` on Windows:

| Plugin | Purpose | Download |
|---|---|---|
| Move Transition | 350 ms scene/source movement and filter animation | https://github.com/exeldro/obs-move-transition/releases |
| Advanced Masks | Rounded-rectangle and circular camera masks | https://github.com/FiniteSingularity/obs-advanced-masks/releases |
| Aitum Vertical Canvas | Independent 9:16 vertical canvas | https://github.com/Aitum/obs-vertical-canvas/releases |
| Background Removal | Camera background removal | https://github.com/royshil/obs-backgroundremoval/releases |
| Multi RTMP | Multi-destination streaming | https://github.com/sorayuki/obs-multi-rtmp/releases |

On OBS 30–32, use the **legacy x64** Advanced Masks package.

The helper scripts in `scripts/` provide checksum-verified installation. Run `scripts/install-plugins.ps1` for a per-user install, or the elevated `scripts/install-plugins-system.ps1` for the system-wide `C:\ProgramData` location required by this machine's OBS setup.

### One-time OBS setup

1. Open **Tools → WebSocket Server Settings** and enable the WebSocket server on port `4455`.
2. Open the **Scene Transitions** dock, click **+**, and add a **Move** transition. `obs-websocket` cannot create transition instances, so discovery will warn if this is missing.
3. Keep the **Move** transition selected; the app sets its duration to 350 ms during bootstrap.

The WebSocket password is read from OBS's own config at runtime, or from the `OBS_WS_PASSWORD` environment variable if set. Never commit the password.

## Build and verify

```powershell
cargo test --workspace
cargo run -p vidyst-obs-driver --bin phase0
cargo run -p vidyst-obs-driver --bin phase0 -- --json
```

`phase0` exits non-zero when OBS is unreachable or a required plugin is missing. The JSON report is written to `docs/phase0-results.json` when run with:

```powershell
cargo run -p vidyst-obs-driver --bin phase0 -- --json |
  Out-File docs\phase0-results.json -Encoding utf8
```

Phase 0 results are recorded in [`docs/phase0-results.md`](docs/phase0-results.md).

## Phase 1 validation

```powershell
# Full run: bootstrap + five tests + cleanup + reports
cargo run -p vidyst-obs-driver --bin spike

# Bootstrap only (no test switching)
cargo run -p vidyst-obs-driver --bin spike -- --bootstrap-only
```

The default `spike` run is idempotent. It creates or reconciles the 8 main and 8 vertical `Vidyst_*` scenes, applies the camera wrapper/mask/morph filters, verifies the live scene graph, runs the five Phase 1 tests, and then restores the original main scene, transition, vertical scene, and audio states. It writes:

- [`docs/phase1-bootstrap.json`](docs/phase1-bootstrap.json) — first bootstrap request log and scene/source UUIDs
- [`docs/phase1-bootstrap-rerun.json`](docs/phase1-bootstrap-rerun.json) — reconnect/idempotency rerun
- [`docs/phase1-results.json`](docs/phase1-results.json) and [`docs/phase1-results.md`](docs/phase1-results.md) — test evidence and screenshots

Phase 1 passed with 8 scenes, 179 requests, 0 failures, and 5/5 tests. If Aitum's vertical scene dock looks stale after the first bootstrap, restart OBS once so it enumerates the raw-created scenes; `switch_scene` already works by name without a restart.

## Phase 2 studio UI

```powershell
# Install once, then start the Tauri app with Vite hot reload
npm install
npm run tauri dev
```

The studio reads the OBS password from the OBS WebSocket config and stores the stream key in Windows Credential Manager. It never logs either secret.

- **Preview:** embedded 15 FPS OBS screenshots for both 16:9 and Aitum 9:16, plus native OBS projector pop-outs.
- **Layouts:** all 8 procedural layouts; when both canvases are selected each card shows a 16:9 plan plus an inset 9:16 portrait plan.
- **Recording:** select 16:9, 9:16, or both. Both writes two independent files. The recording bar shows the folder, last file, and target canvases.
- **Sources:** camera and screen Connect/Disconnect controls release the hardware independently of the eye visibility toggle. Selections and connection state persist.
- **Streaming:** 16:9 uses the main OBS output, 9:16 uses Aitum's output, and both start separately. Add a vertical stream output/destination in the Aitum Vertical dock before testing 9:16 streaming.
- **Known limitation:** Multi-RTMP has no configured destinations yet; see `P2-45` in `TASKS.md`.

## Workspace layout

```text
crates/
  layouts/       Pure layout geometry (no OBS dependency)
  obs-driver/    obws client, discovery, Phase 1 bootstrap, and validation harness
scripts/         Plugin installation helpers
docs/            Phase reports
backups/         OBS scene-collection backups (local only)
```

## Scope guardrails

- No custom OBS plugin and no hand-written scene-collection JSON.
- No live chat, guest/backstage rooms, overlay servers, or social stats in V1.
- 16:9 screens inside 9:16 layouts always use `Fit::Contain`, never `Fit::Cover`.
- No OBS Virtual Camera preview; use the native OBS projector.

## Troubleshooting

- **OBS unreachable:** confirm OBS is running and the WebSocket server is enabled on `4455`.
- **Plugin reported missing:** verify the plugin folder is under `C:\ProgramData\obs-studio\plugins` and restart OBS.
- **Move transition missing:** add a **Move** transition manually in the Scene Transitions dock; this cannot be automated through the current OBS API.
- **Advanced Masks not detected:** make sure the legacy x64 build is installed; OBS's built-in mask filter is not the plugin.
- **Aitum vertical scene list is stale:** restart OBS once after the first bootstrap. Aitum builds its dock list at startup; `switch_scene` still works by name in the meantime.
