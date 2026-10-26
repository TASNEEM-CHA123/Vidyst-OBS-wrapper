# Vidyst-OBS

A lightweight companion studio controller for OBS Studio. OBS remains the capture, compositing, and encoding engine; this app drives OBS through `obs-websocket` v5.

> **Current status:** Phase 0 and Phase 1 are complete. Phase 2 is **local V1 verified** (2026-10-26): the Tauri/Svelte studio UI, live embedded previews, audio meters, aspect-aware recording, camera/screen controls, dual-canvas recording, and local H/V streaming are working. Release installer/clean-profile smoke testing and final user sign-off remain. See [`docs/phase2-results.md`](docs/phase2-results.md).

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
# Install once
npm install

# Recommended: isolated dev collection + .dev-state
.\scripts\dev.ps1
# or manual
npm run tauri dev
```

`scripts/dev.ps1` sets `VIDYST_CONFIG_DIR=.dev-state`, switches OBS to `Vidyst_Dev`, and starts Tauri. Test recordings go to `.dev-recordings`; both folders are gitignored. Use `.\scripts\dev-reset.ps1` and `devctl restore --state .dev-state/original.json` to reset/restore the real OBS collection.

The studio reads the OBS password from the OBS WebSocket config and stores stream keys in Windows Credential Manager. It never logs either secret. App-level `stream_server` is intentionally `null`; OBS/Aitum own destinations.

- **Preview:** 640 px-wide `GetSourceScreenshot` JPEGs (16:9 640x360, 9:16 360x640), default 15 FPS, configurable 5-30 FPS; pauses when hidden and backs off to ~2 FPS when OBS render health degrades. Native OBS projectors remain available. No Virtual Camera.
- **Layouts:** all 8 procedural layouts; when both canvases are selected each card shows a 16:9 plan plus an inset 9:16 portrait plan.
- **Recording:** select 16:9, 9:16, or both. Both writes two independent files. The recording bar shows the folder, last file, and target canvases.
- **Split recording:** tries OBS `SplitRecordFile` first, then falls back to close/reopen. The fallback logs its path and shows a warning because it is not gapless on OBS 32.2.2 (measured 1.3-1.6 s for 16:9; 9:16 handoff ~45-70 ms).
- **Background removal:** created once at bootstrap, toggled only; missing filter shows **Unavailable** instead of failing silently. Visual cutout sign-off is still pending.
- **Sources:** camera and screen Connect/Disconnect controls release the hardware independently of the eye visibility toggle. Selections and connection state persist. Missing camera shows a "Camera disconnected" placeholder rather than a black rectangle.
- **Streaming:** 16:9 uses the main OBS output, 9:16 uses Aitum's output, and both start/stop independently. Live tests used local MediaMTX relays on ports 1935/1936. Multi-RTMP targets and public/private RTMP destinations are not configured yet.
- **State machine:** `Idle -> Starting -> Live -> Stopping -> Error`, plus `Degraded` with retry when one target is live and the other fails. Stop requests run against both outputs concurrently.
- **Soak:** 31m 8s / 60 samples; both outputs stayed active with no reconnects or MediaMTX errors. Desktop load caused 2.6% horizontal skipped frames and 10.1% render skips, so this is a stability pass, not a zero-drop pass.
- **Validation:** see [`docs/phase2-results.md`](docs/phase2-results.md) for stream cases, split files, soak metrics, restore evidence, and known limitations.
- **Restore caveat:** OBS 32.2.2 crashed once while switching back to the real collection with this plugin set. Restarting OBS directly on the target collection is safer; the actual restoration completed and was verified.

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
- **Split shows a gap warning:** expected on OBS 32.2.2. `SplitRecordFile` is rejected for this output, so the app uses close/reopen fallback and reports `gapless=false`. See `docs/phase2-results.md`.
- **Streaming test fails:** confirm both MediaMTX relays are running on `1935` and `1936` and that the Aitum vertical output has a destination. The app reports "configured" but does not store keys.
- **Test state leaked into the real collection:** stop the app, then run `devctl restore --state .dev-state/original.json`; restore the Aitum config backup separately if present.
