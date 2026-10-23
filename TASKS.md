# Vidyst-OBS — V1 Build Task Tracker

> **Goal:** Build a Vidyst-style studio controller as a lightweight companion app on top of OBS Studio.
> **Source docs:** [`project.md`](./project.md) · [`tech_stack.md`](./tech_stack.md) · [`phase.md`](./phase.md)
> **Created:** 2026-10-22
> **Platform:** Windows 11 26H2 (verified)
> **Status:** **Phase 0 complete** — **Phase 1 complete** — **Phase 2: in progress (V1 demo working; streaming destinations still pending)**

---

## How to use this file

- Tick tasks by changing `- [ ]` to `- [x]`.
- Work through phases **in order**. Do not start a phase until the previous phase gate is ticked.
- Use the task IDs in commit messages, issues, and notes (e.g. `P1-06`).
- Anything marked **Deferred** is Phase 4 / out of V1 and must not be built yet.

### Phase dashboard

| Phase | Scope | Gate | Status |
|---|---|---|---|
| **0** | Environment discovery, OBS connection, plugin verification, Rust scaffold | OBS reachable + plugins checked + workspace builds | ✅ Passed |
| **1** | OBS bootstrap spike + 5 validation tests | All validation tests pass | ✅ Passed |
| **2** | Tauri v2 + Svelte 5 studio UI and full V1 feature wiring | End-to-end acceptance demo | 🟡 In progress |
| **4** | Live chat, guests, overlays, stats | Future — do not start | 🚫 Deferred |

---

## 1. V1 scope lock

### Must ship in V1

- [ ] 8 one-click animated layouts (`just_me`, `screen_cam`, `screen_cam_inv`, `screen_tile`, `screen_tile_inv`, `side_by_side`, `side_by_side_inv`, `screen_focus`).
- [ ] Horizontal 16:9 and Vertical 9:16 support, selectable together, with 1:1 and 4:5 kept extensible.
- [ ] Record (Hybrid MP4 + automatic file splitting) and Go Live (stream key in OS keychain).
- [ ] Camera / phone / screen source switching and visibility toggles.
- [ ] External mic + screen audio: capture mute, listen/monitor, volume.
- [ ] Background removal toggle + camera background image/color.
- [ ] Quick camera resize: corner snap, size slider, circle ↔ tile, side-by-side split ratio.
- [ ] Save/load custom layout presets as JSON.
- [ ] Zero-latency preview via OBS projector.
- [ ] OBS connection health + reconnect handling.

### Explicitly out of scope for V1 (guardrails)

- [ ] **Do not** build a custom video compositor, audio clock, or encoder — OBS is the engine.
- [ ] **Do not** write an in-process C/C++/Rust OBS plugin — use `obs-websocket` v5 + `obws` only.
- [ ] **Do not** hand-write OBS scene-collection JSON — bootstrap everything through WebSocket requests.
- [ ] **Do not** build live chat, local overlay servers (`axum`), or guest/backstage rooms — Phase 4.
- [ ] **Do not** default a 16:9 Screen slot to `Fit::Cover` inside a 9:16 layout — use `Fit::Contain`.
- [ ] **Do not** use OBS Virtual Camera for preview — use `OpenVideoMixProjector`.
- [ ] **Do not** proceed without verifying OBS, WebSocket connectivity, and plugins first.

---

## 2. Screenshot → V1 parity map

The reference screenshot shows more than the V1 spec allows. This table keeps the visual target clear without violating the scope lock.

| Screenshot element | V1 behavior | Status |
|---|---|---|
| `● LIVE` / `● REC` / idle badge | Driven by OBS streaming/recording events | ✅ In V1 |
| Active layout name (`Side by side inverted`) | Updates after every layout switch | ✅ In V1 |
| `1,256 watching · 312 likes` | Vidyst cloud stats; no equivalent in local OBS | 🚫 Omit / deferred |
| `Horizontal` / `Vertical` toggles | Multi-select canvas targets + 1:1 / 4:5 menu | ✅ In V1 |
| Gear icon | Settings: OBS host/port/password, recording path, plugin health | ✅ In V1 |
| Large live preview area | Button opens native OBS projector (not embedded video) | ✅ In V1 |
| Live chat panel + message input | Phase 4 feature | 🚫 Deferred |
| Right rail: Chat / Backstage / Guests | Phase 4 features | 🚫 Deferred |
| Right rail: Audio / Edit | Audio mixer and quick-customize panels | ✅ In V1 |
| `Healthy` indicator | OBS WebSocket connection health | ✅ In V1 |
| `Layouts` dock with SOLO / SCREEN groups | 8 procedural layout cards with exact slot rects | ✅ In V1 |
| Purple screen / brown camera thumbnails | CSS/SVG thumbnails using documented colors | ✅ In V1 |
| `Auto-switching` toggle | Behavior not defined in spec | ⏸️ Backlog until defined |
| Chat/Backstage/Guests icons | Hide by default; optional disabled tooltip | 🚫 Deferred |

> **Decision needed:** keep the chat sidebar as a disabled visual placeholder for screenshot parity, or hide it entirely. Recommended: hide it in V1.

---

## 3. PHASE 0 — Environment discovery & OBS verification

> Reference: `phase.md` §PHASE 0, `tech_stack.md` §3.

### 3.1 Machine & OBS discovery

- [x] **P0-01** Record environment: Windows 11 26H2 build 26300, i5-1135G7, Iris Xe, 42.5 GB free, working directory `E:\Reseach Projects\vidyst-obs`. Saved in `docs/phase0-results.md`.
- [x] **P0-02** Locate OBS binary: `C:\Program Files\obs-studio\bin\64bit\obs64.exe`.
- [x] **P0-03** Confirm OBS version is **31.1+ / 32.x** — verified **32.2.2**.
- [x] **P0-04** Confirm `obs-websocket` v5 is enabled: **Tools → WebSocket Server Settings** → enabled on port `4455`.
- [x] **P0-05** WebSocket auth is enabled; password is read from OBS's config at runtime (or `OBS_WS_PASSWORD`) and never committed. Report records only `ObsConfig`.
- [x] **P0-06** Verify port reachability: `Test-NetConnection 127.0.0.1 -Port 4455`; `obws` connects successfully.
- [x] **P0-07** Record the plugin directory: `C:\ProgramData\obs-studio\plugins` (plus built-in `C:\Program Files\obs-studio\obs-plugins\64bit`).

### 3.2 Plugin verification

Check each required plugin and note its version/status:

- [x] **P0-08** `obs-move-transition` 3.2.1 — `move_value_filter` / `move_source_filter` / `move_transition` kinds; DLL loaded.
- [x] **P0-09** `obs-advanced-masks` 1.6.0 legacy x64 — `advanced_masks_filter` / `advanced_masks_filter_v2` kinds with corner-radius support. (`mask_filter` is OBS's built-in filter, not this plugin.)
- [x] **P0-10** `Aitum Vertical` 1.6.6 — `Aitum Vertical` 1080x1920 canvas present; vendor `version` request works.
- [x] **P0-11** `obs-backgroundremoval` 1.4.1 — `background_removal` filter kind; DLL loaded.
- [x] **P0-12** `obs-multi-rtmp` 0.7.4.4 — system plugin DLL loaded; directory and DLL verified.
- [x] **P0-13** Exact OS-specific install instructions are in `README.md` and `scripts/`; no plugins are currently missing.
- [x] **P0-14** Resolved spec inconsistency: all **5** third-party plugins + built-in WebSocket verified. `phase.md`'s “4 plugins” wording is outdated.

### 3.3 Workspace scaffold

- [x] **P0-15** Root `Cargo.toml` workspace (`resolver = "2"`) with release profile: `lto = "fat"`, `codegen-units = 1`, `strip = true`.
- [x] **P0-16** `crates/layouts` scaffolded — pure Rust, **no OBS dependency**.
- [x] **P0-17** Implemented `AspectRatio`, `LayoutGroup`, `SourceId`, `Fit`, `Rect`, `Slot`, `Layout` per `tech_stack.md` §4.
- [x] **P0-18** Encoded all 8 built-in layouts for `R16x9` and `R9x16` with the exact normalized coordinates in `tech_stack.md` §4.
- [x] **P0-19** Unit tests added and passing:
  - [x] Circle slot produces equal pixel width/height on both canvases.
  - [x] Vertical screen slots always use `Fit::Contain`.
  - [x] Every layout covers the expected slots and hidden items are represented.
  - [x] Normalized rect values stay within `0.0..=1.0`.
- [x] **P0-20** `crates/obs-driver` scaffolded with `obws` 0.15, `tokio`, `serde`, `anyhow`, `tracing`.
- [x] **P0-21** `phase0` discovery binary connects and prints OBS/WebSocket versions, input/filter/transition kinds, canvases, credentials source, and plugin status.
- [x] **P0-22** Discovery binary exits non-zero when OBS is unreachable or a required plugin is missing.
- [x] **P0-23** `README.md` written with OBS setup, plugin links, password handling, discovery commands, and troubleshooting.
- [x] **P0-24** Verified `cargo test --workspace` (11 passed) and `cargo run -p vidyst-obs-driver --bin phase0` (exit 0, `ready: true`).

### Phase 0 gate

- [x] **P0-GATE** OBS is reachable on `4455`, all required plugins are installed and verified, all 11 layout/discovery tests pass, and the discovery binary reports readiness. Report: `docs/phase0-results.md`.

---

## 4. PHASE 1 — OBS bootstrap spike & validation tests

> Reference: `phase.md` §PHASE 1.
> **Before starting:** use a throwaway OBS profile/scene collection or ask the user to back up their current scene collection. The spike creates scenes and sources.

### 4.1 Programmatic bootstrap

- [x] **P1-01** Create `crates/obs-driver/src/bin/spike.rs` with host/port/password options and idempotent setup (safe to run repeatedly).
- [x] **P1-02** Create raw camera input `Vidyst_Camera_Raw` and wrapper scene `Vidyst_Camera_Group`; filters attach to the wrapper so they affect the camera cleanly.
- [x] **P1-03** Add `obs-advanced-masks` rectangle mask with configurable corner radius (default 8%).
- [x] **P1-04** Add `Move Value` filters:
  - [x] `Morph_To_Circle` — 50% radius over 350 ms, triggered on enable.
  - [x] `Morph_To_Tile` — 8% radius over 350 ms, triggered on enable.
  - [x] Verify filters can be toggled repeatedly without stacking or leaving a stale radius.
- [x] **P1-05** Create the 8 wrapper scenes: `Vidyst_just_me`, `Vidyst_screen_cam`, `Vidyst_screen_cam_inv`, `Vidyst_screen_tile`, `Vidyst_screen_tile_inv`, `Vidyst_side_by_side`, `Vidyst_side_by_side_inv`, `Vidyst_screen_focus`.
- [x] **P1-06** Add `Vidyst_Screen` and `Vidyst_Camera_Group` to every scene and apply transforms from the `Slot` math:
  - [x] `alignment = TopLeft`, `bounds_alignment = Center`.
  - [x] `Fit::Cover` → `ScaleToOuterBounds` + `crop_to_bounds = true`.
  - [x] `Fit::Contain` → `ScaleToInnerBounds` + `crop_to_bounds = false`.
  - [x] Hide the screen in `just_me` and hide the camera in `screen_focus` via `SetSceneItemEnabled(false)`.
  - [x] Apply the correct 16:9 and 9:16 slot sets.
- [x] **P1-07** Set OBS's current scene transition to `Move` with duration `350 ms`; verify it exists before applying.
- [x] **P1-08** Persist bootstrap results/IDs in memory (not hand-written JSON) and log every request result.

### 4.2 Validation tests

- [x] **P1-09 Test 1 — Circle ↔ Tile morph:** switch `Vidyst_screen_cam` (circle) → `Vidyst_side_by_side` (tile), enabling `Morph_To_Tile`. Confirm smooth 350 ms movement and radius animation. **Result:** passed; radius `540 → 86.4` over 16 samples, settled at `~328 ms` with a Move event span inside the expected range.
- [x] **P1-10 Test 2 — Rapid click test:** switch across 5 layouts in 1 second (200 ms apart). Verify animation interpolates mid-flight with no glitching. **Result:** passed; all 5 requests completed in `926 ms` on a 200 ms cadence, 5 transition starts and 3 mid-flight radius samples observed, final scene/radius settled correctly.
- [x] **P1-11 Test 2 fallback:** if scene switching glitches, document and evaluate the **single master scene + `Move Source` filter** fallback. Do not change architecture without recording the result. **Result:** not needed; no glitch observed.
- [x] **P1-12 Test 3 — Audio toggles:** test `set_muted` and `set_audio_monitor_type(MonitorType::MonitorAndOutput)` on both mic and screen audio; restore original values after the test. **Result:** passed on both inputs; original states restored.
- [x] **P1-13 Test 4 — Vertical sync:** switch the vertical scene through Aitum Vertical linked scenes or `call_vendor_request("aitum-vertical-canvas", "switch_scene", ...)`. Capture the actual vendor request/response; document a fallback if the API differs. **Result:** passed; `switch_scene` matched `current_scene` for all 8 vertical scenes. One OBS restart populated Aitum's `get_scenes` dock; `switch_scene` is the supported switching path.
- [x] **P1-14 Test 5 — Spec gap:** `phase.md` says "5-test validation" but only lists 4. Proposed fifth test: **disconnect/reconnect + re-run bootstrap without duplicate sources, then restore the original OBS scene/transition.** Confirm or replace before running. **Confirmed and passed:** reconnect + rerun produced 179 requests, 0 failures, stable UUIDs and no duplicate inputs.
- [x] **P1-15** Record all test results, screenshots, logs, and observed OBS/plugin versions in `docs/phase1-results.md`. **Report:** [`docs/phase1-results.md`](docs/phase1-results.md), JSON [`docs/phase1-results.json`](docs/phase1-results.json), 27 screenshots under `docs/screenshots/phase1/`.
- [x] **P1-16** Clean up or preserve spike scenes based on user preference; leave OBS in a usable state. **Result:** scenes/sources preserved; original main scene `Scene`, transition `Fade 300 ms`, vertical `Vertical Scene`, and audio states restored. Temporary probes removed.

### Phase 1 gate

- [x] **P1-GATE** All 5 tests pass (or fallbacks are explicitly accepted), no duplicate sources after rerun, and Phase 1 results are documented. **Passed 2026-10-22:** `Phase 1: PASS`, 8 scenes, 179 bootstrap requests, 0 failures, all 5 tests pass, cleanup restored.

---

## 5. PHASE 2 — Tauri v2 + Svelte 5 studio UI

> Reference: `phase.md` §PHASE 2, `tech_stack.md` §3 workspace layout.

### 5.1 App shell & bridge

- [x] **P2-01** Scaffold `src-tauri` (Tauri v2) and `ui` (Svelte 5 + Vite + TypeScript).
- [x] **P2-02** Build the dark three-zone shell: top bar, main preview area, right rail, bottom layout dock.
- [x] **P2-03** Define typed Tauri commands and events:
  - [x] `apply_layout`
  - [x] `quick_resize_camera`
  - [x] `set_audio_*` (mute, monitor, volume)
  - [x] `toggle_background_removal`
  - [x] `list_devices` / `set_camera_source` / `set_screen_source`
  - [x] `start_recording` / `stop_recording`
  - [x] `start_streaming` / `stop_streaming`
  - [x] `open_projector`
  - [x] `save_layout_preset` / `load_layout_preset`
  - [x] Connection/status events for UI state.
- [x] **P2-04** Implement one persistent `obws` connection with auto-reconnect/backoff and OBS event subscriptions.
- [x] **P2-05** Add `keyring` integration for stream keys; never log credentials.
- [x] **P2-06** Add connection error surfaces and a disconnected state so UI actions cannot silently fail.

### 5.2 Top bar

- [x] **P2-07** Status badge reflects `LIVE` / `REC` / idle from OBS events (not optimistic UI guesses).
- [x] **P2-08** Show the active layout display name (`Side by side inverted`, etc.).
- [x] **P2-09** Aspect ratio multi-select: `Horizontal 16:9`, `Vertical 9:16`, plus `1:1` / `4:5` menu; apply to both canvases as selected.
- [x] **P2-10** Record button: start/stop Hybrid MP4 (fallback MKV) with `SplitRecordFile` and configurable output path. **Verified:** split falls back to a close/reopen boundary when Hybrid MP4 rejects `SplitRecordFile`; both canvases produced two files per side after a split.
- [x] **P2-10b** Visible recording path in the main UI, native folder picker (`rfd`) that applies `SetRecordDirectory`, last saved filename from `RecordStateChanged`, and Open in Explorer. **Verified:** folder/last-file display, picker wiring, Explorer launch, and vertical-only last-file tracking (16:9 and 9:16 files validated).
- [ ] **P2-11** Go Live button: start/stop streaming, load key from keychain, sync Aitum Vertical + `obs-multi-rtmp` destinations. **Partial:** Aitum vendor destination sync and per-aspect start/stop are implemented; a successful end-to-end stream is blocked until Aitum contains a stream output and a destination/key is configured. Multi-RTMP has no configured targets.
- [x] **P2-42** Aspect-aware recording/streaming: 16:9 only records/streams the main OBS output, 9:16 only uses the Aitum vertical output, and both produce two independent horizontal/vertical outputs. **Verified for recording:** separate `*.mp4` and `*-vertical.mp4` files, flags, and timecodes; streaming start/stop reports clear failures when no destination exists.
- [x] **P2-44** Per-output REC/LIVE indicators (`H`, `V`, `H+V`) and vertical last-file tracking so 9:16-only recordings appear in the recording bar.
- [x] **P2-12** Preview button: open native OBS projector via `OpenVideoMixProjector` for horizontal/vertical — **never** Virtual Camera.
- [x] **P2-12b** Embedded live video preview in the center stage: 15 FPS JPEG frame streamer over `GetSourceScreenshot` for 16:9 and Aitum 9:16, with a schematic fallback only while disconnected. **Verified:** live 960×540 and 540×960 captures, including UUID-based vertical scene capture.
- [x] **P2-13** Settings modal: OBS host/port/password, recording format/path, plugin health report, clear stored stream key.
- [x] **P2-14** `Healthy` indicator reflects WebSocket connection state.

### 5.3 Bottom layout dock

- [x] **P2-15** Render 8 layout cards grouped under `SOLO` and `SCREEN`, procedurally from `crates/layouts` data (no static image assets).
- [x] **P2-16** Screen slots in muted purple (`#4b4863`); camera slots in warm brown (`#6b5b4b`); border radius follows `radius_pct`.
- [x] **P2-17** Thumbnail aspect follows the preview target (16:9 or 9:16). **Updated:** when both canvases are selected every card shows a 16:9 plan plus an inset 9:16 plan; the 9:16 plan itself remains stacked.
- [x] **P2-43** Dual-aspect layout presentation requested by the user: 16:9 and 9:16 are visible together when both are selected, while 9:16 remains a stacked portrait plan rather than a side-by-side plan.
- [x] **P2-18** Clicking a card calls `invoke("apply_layout", { id })`, updates the active name, and highlights the card.
- [x] **P2-19** Handle rapid clicks gracefully (consistent with Test 2 result).
- [x] **P2-20** `Auto-switching` toggle from the screenshot: **deferred** unless the user defines its behavior. **Resolved:** omitted; tracked as `F-05`.
- [x] **P2-21** Optionally expose a vertical-preview toggle for layout thumbnails. **Resolved:** the dock now follows the selected canvas targets automatically; both selected shows dual 16:9 + inset 9:16 plans.

### 5.4 Devices, background & quick resize

- [x] **P2-22** Camera picker: local webcam, phone/Continuity/DroidCam/Camo, or VDO.Ninja browser source; visibility toggle.
- [x] **P2-23** Screen picker: full display or specific application window; visibility toggle.
- [ ] **P2-24** Background removal toggle via `SetSourceFilterEnabled`; background image/color layer behind camera. **Background color/image verified.** Background-removal filter remains disabled pending visual validation because the installed plugin previously produced a white/transparent frame.
- [x] **P2-41** Explicit camera and screen Connect/Disconnect controls, separate from the eye visibility buttons. State and device selections persist; bootstrap reconnects or releases devices, and disconnecting the camera clears its device ID so the camera is not held indefinitely. **Verified:** release/reconnect and persistence across an app restart.
- [x] **P2-25** Quick resize controls: corner snap `TL/TR/BL/BR`, camera size slider, `Circle ↔ Tile` toggle, side-by-side split ratio.
- [x] **P2-26** Throttle live transform updates to ~30–60 Hz (30 ms), then send one final exact transform on pointer release.
- [x] **P2-27** Save current customization as a reusable JSON preset; load/validate through `crates/layouts` and apply via WebSocket.

### 5.5 Audio mixer

- [x] **P2-28** Two rows: `External Mic` and `Screen Audio`.
- [x] **P2-29** Each row has capture mute/unmute (`SetInputMute`), listen on/off (`MonitorAndOutput` ↔ `None`), and volume slider in dB (`SetInputVolume`).
- [x] **P2-30** Reflect actual OBS state from events rather than only local UI state.
- [x] **P2-30b** Live bouncing VU meters (`INPUT_VOLUME_METERS`, ~20 Hz) inside the Mic and Screen Audio rows, from −60 dB to 0 dB. **Verified:** meter events throttled to ≥40 ms and rendered for both inputs.
- [x] **P2-31** Add a headphones/feedback warning when monitoring is enabled on speakers.

### 5.6 Right rail & screenshot alignment

- [x] **P2-32** `Audio` icon opens the mixer; `Edit` icon opens quick-customize; gear opens settings.
- [x] **P2-33** Hide chat/backstage/guests icons by default, or show them disabled with a "Phase 4" tooltip. Do **not** implement chat.
- [x] **P2-34** Omit watcher/like counters (no data source in local OBS). Optionally reserve space for a future stats integration.

### 5.7 Acceptance & polish

- [ ] **P2-45** Add a vertical stream output + test destination in Aitum Vertical and configure Multi-RTMP targets, then validate 16:9-only, 9:16-only, and both-canvas live streaming. **Blocked:** Aitum `stream_outputs` is empty and no RTMP destination/key is configured; start-failure paths are verified.
- [ ] **P2-35** End-to-end test: apply all 8 layouts on 16:9 and 9:16; toggle audio; remove background; quick-resize; record; stream to a test key; reconnect OBS.
- [ ] **P2-36** Verify visual parity with the reference screenshot except deferred chat/stats items.
- [ ] **P2-37** Performance check: near-zero idle CPU, reasonable RAM, fast startup; note release binary/installer size.
- [ ] **P2-38** Handle window resizing and small screen sizes without breaking the layout dock.
- [x] **P2-39** README: install, OBS setup, plugins, troubleshooting, and known limitations. **Updated** with Phase 2 run instructions, feature notes, and the pending streaming-destination limitation.
- [ ] **P2-40** Build a release installer and smoke-test it on a clean Windows profile if possible.

### Phase 2 gate

- [ ] **P2-GATE** Full V1 demo passes acceptance, docs are updated, and the user signs off on the implementation.

---

## 6. Phase 4 backlog — deferred, do not build

- [ ] **F-01** Live chat across YouTube / Twitch / X destinations + chat moderation.
- [ ] **F-02** Viewer count and like/reaction stats integration.
- [ ] **F-03** Backstage and remote guest/podcast rooms.
- [ ] **F-04** Local web overlay server (`axum`).
- [ ] **F-05** Auto-switching layouts.
- [ ] **F-06** Multi-destination chat/stream orchestration beyond `obs-multi-rtmp`.

---

## 7. Decisions & open questions

- [x] **D-01** Chat sidebar in the screenshot: hide in V1 (recommended) or show a disabled placeholder? **Resolved:** hide entirely in V1; chat/backstage/guests are Phase 4.
- [x] **D-02** Watcher/like counters: omit (recommended) or add non-functional placeholders? **Resolved:** omit; no local OBS data source.
- [x] **D-03** Define Phase 1 Test 5: reconnect + idempotency (proposed)? **Resolved:** implemented and passed; see `docs/phase1-results.md`.
- [x] **D-04** Auto-switching: omit from V1 or define exact behavior? **Resolved:** omit from V1; tracked as backlog `F-05`.
- [x] **D-05** Confirm all 5 third-party plugins are mandatory for V1, despite `phase.md` saying 4. **Resolved:** all five are installed and verified; `phase.md` wording is treated as outdated.
- [x] **D-06** Default recording format: Hybrid MP4 (recommended) with MKV fallback? **Resolved:** Hybrid MP4 default; MKV/MP4 fallbacks remain selectable.
- [x] **D-07** Confirm it is safe to create `Vidyst_*` scenes in the current OBS profile, or use a backup/throwaway profile. Scene collection backup exists at `backups/Untitled-20261022-032942.json`. **Resolved:** user approved the backed-up current profile; Phase 1 scenes/sources are preserved.
- [x] **D-08** Preview approach: external OBS projector in V1 (recommended) vs embedding preview later. **Resolved:** embed a 15 FPS `GetSourceScreenshot` JPEG stream in the center stage for both horizontal and Aitum vertical canvases; the native projector remains a secondary pop-out. No Virtual Camera.

---

## 8. Risks / watch items

| Risk | Impact | Mitigation |
|---|---|---|
| `obs-advanced-masks` settings differ between plugin versions | Circle/tile mask may not apply correctly | Probe filter properties live in Phase 1; pin supported versions |
| Aitum Vertical vendor API is undocumented/changes | Vertical scene sync fails | Capture actual request/response; add linked-scene fallback |
| `Move` transition absent or misconfigured | No 350 ms animation | Check transition list during discovery; fail clearly |
| Rapid scene switching glitches | Broken visual transitions | Use Test 2 fallback to single master scene + Move Source filters |
| WebSocket auth / port differences | App cannot connect | Settings modal + clear connection diagnostics |
| Hybrid MP4 unsupported | Recording failure | Fall back to MKV and surface status |
| Spike modifies user's OBS setup | Lost scenes/settings | Back up scene collection before Phase 1 |

---

## 9. Progress log

| Date | Task ID | Change / result | Notes |
|---|---|---|---|
| 2026-10-22 | — | Task tracker created from `project.md`, `tech_stack.md`, `phase.md` | Phase 0 not started |
| 2026-10-22 | P0-01..P0-22 | Rust workspace, 8 dual-ratio layouts, discovery CLI, and plugin verification implemented | `cargo test --workspace`: 11 passed |
| 2026-10-22 | P0-08..P0-12 | All five required plugins installed system-wide and loaded by OBS 32.2.2 | Move, Advanced Masks, Aitum Vertical, Background Removal, Multi RTMP |
| 2026-10-22 | P0-09 | Advanced Masks detection corrected to `advanced_masks_filter` / `advanced_masks_filter_v2` | Built-in `mask_filter` was a false positive |
| 2026-10-22 | P0-23, P0-24, P0-GATE | README and Phase 0 report written; discovery ready; Phase 0 gate passed | `docs/phase0-results.md`, `docs/phase0-results.json` |
| 2026-10-22 | P1-07 (partial) | Added a `Move` transition instance manually; WebSocket API cannot create transition instances | Discovery now reports the instance and warns if missing |
| 2026-10-22 | P1-01 | Phase 1 scene/source bootstrap implemented; scene collection and WebSocket config backed up | Full test runner added to `spike.rs` |
| 2026-10-22 | P1-01..P1-08 | Idempotent bootstrap creates 8 main + 8 vertical scenes, camera wrapper, Advanced Masks + Move Value filters, camera fill, and Move 350 ms | `docs/phase1-bootstrap.json`: 179 requests, 0 failed |
| 2026-10-22 | P1-09..P1-13 | Tests 1–4 passed: circle/tile morph, rapid 200 ms cadence, audio toggles, Aitum vertical sync | OBS restart populated Aitum `get_scenes`; `switch_scene` verified for all 8 |
| 2026-10-22 | P1-14..P1-16 | Test 5 disconnect/reconnect + idempotent rerun passed with stable UUIDs; reports written; probes removed | Cleanup restored `Scene`, `Fade 300 ms`, `Vertical Scene`, and audio |
| 2026-10-22 | P1-GATE | Phase 1 gate passed | `Phase 1: PASS`, 5/5 tests, 0 failed requests, 27 screenshots |
