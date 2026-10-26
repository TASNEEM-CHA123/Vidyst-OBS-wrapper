# Phase 2 Finish — Immediate Execution Plan

> **Status:** LOCAL V1 VERIFIED (2026-10-26); release installer and `P2-GATE` sign-off pending.
> **Scope:** Finish Phase 2 to a signed-off V1 demo. This file is the working checklist; `TASKS.md` remains the master tracker.
> **Evidence:** [`docs/phase2-results.md`](./docs/phase2-results.md).
> **Locked decisions**
> - **Preview:** keep the verified 15 FPS `GetSourceScreenshot` JPEG stream. Do **not** add OBS Virtual Camera. The native OBS projector remains the smooth full-frame option. Both canvases stay visible simultaneously.
> - **Streaming comes early**, not last. Validate locally with MediaMTX first, then one private/unlisted stream and a 30-minute soak.
> - **Secrets never enter the repo.** OBS/Aitum/Multi-RTMP hold destinations and keys; the app only reports "configured" and starts/stops. `backups/` and `.env` are gitignored.
> - **Dev isolation:** dedicated OBS scene collection (`Vidyst_Dev`), dedicated app-data folder (`VIDYST_CONFIG_DIR=.dev-state`), and a reset script. Test state must never touch the real `settings.json` again.
> - **Screenshot preview is a performance cost:** ~640 px wide, configurable rate, pause when hidden or when OBS render health degrades.
> - **Background removal is core**, not a small gap.
> - **Missing files degrade gracefully** (missing background image → safe fallback + warning, never a broken source).
> - **Split recording must be gapless.** Log which path ran; verify joined durations.

---

## Step 0 — Repo baseline & secret hygiene

- [x] **P2F-00a** Create comprehensive `.gitignore`: secrets, `backups/`, `target/`, `node_modules/`, `ui/dist`, `src-tauri/gen`, logs, OS noise.
- [x] **P2F-00b** Create `.env` (untracked) + `.env.example` (tracked) for local OBS/RTMP overrides.
- [x] **P2F-00c** Triple-check staged content: no real OBS WebSocket password, no stream keys, no `backups/`, no `.env`.
- [x] **P2F-00d** `git init`, baseline commit `dc0a224`, tag `phase2-wip`, push to `TASNEEM-CHA123/Vidyst-OBS-wrapper` (`main`).
- [x] **P2F-00e** Keep committing each step below; never commit `.dev-state/`, recordings, or OBS backups.

**Acceptance:** `git status` clean; `git ls-files` contains no secret-bearing backup; remote `main` matches local.

---

## Step 1 — Verification baseline (no OBS required)

- [x] **P2F-01a** `npm --prefix ui run check` → 0 errors / 0 warnings.
- [x] **P2F-01b** `npm --prefix ui run build` → success.
- [x] **P2F-01c** `cargo fmt --all -- --check` → initially failed; applied rustfmt (`ebbee73`); now clean.
- [x] **P2F-01d** `cargo check --workspace --all-targets` → success.
- [x] **P2F-01e** `cargo test --workspace` → 11 passed (9 layouts + 2 discovery).

**Acceptance:** all five commands pass on the current tree.

---

## Step 2 — Dev isolation + clean state

- [x] **P2F-02a** Add `VIDYST_CONFIG_DIR` override in `src-tauri/src/settings.rs`; gitignore `.dev-state/`.
- [x] **P2F-02b** Add `scripts/dev.ps1`: sets `VIDYST_CONFIG_DIR=.dev-state`, launches `npm run tauri dev`.
- [x] **P2F-02c** Add `devctl` CLI (`crates/obs-driver/src/bin/devctl.rs`):
  - `save-original` — record current scene collection, scene, transition, vertical scene, audio state in `.dev-state/original.json`.
  - `setup-dev` — create/switch `Vidyst_Dev`, run the existing bootstrap.
  - `reset` — clean the active collection (background off, display capture, camera connected, Fade 300, program scene, vertical scene).
  - `restore` — switch back to the original collection and restore the saved state.
  - `status` / `probe` — print collection, scene, transition, vertical, vendor responses.
- [x] **P2F-02d** Add `scripts/dev-reset.ps1`: stops/starts nothing destructively; cleans `.dev-state/settings.json` and calls `devctl reset`.
- [x] **P2F-02e** Clean the real profile once: background off, screen → display capture, camera reconnected, temp image cleared, program `Scene`, `Fade 300 ms`, vertical `Vertical Scene`.
- [x] **P2F-02f** Delete the temp background image (`%TEMP%\vidyst-test-bg.png` if present) and verify startup falls back without a broken source.

**Acceptance:** test runs write only to `Vidyst_Dev` + `.dev-state`; the real collection/settings are untouched; reset script returns OBS to the documented clean state.

---

## Step 3 — Streaming bring-up (early)

- [x] **P2F-03a** Add a streaming state machine in `ObsService`: `Idle → Starting → Live → Stopping → Error`, plus `Degraded` when one target is live and the other failed. Persist requested targets and per-target errors; expose `stream_state`, `stream_error`, `stream_degraded_targets` in `StudioSnapshot`.
- [x] **P2F-03b** Partial-failure policy: horizontal success + vertical failure must stay **LIVE (degraded)**, not stop horizontal. UI shows degraded badge + Retry (retries only the failed target).
- [x] **P2F-03c** UI: TopBar reflects `Starting / Live / Degraded / Stopping / Error`, with retry and clear error text. Never optimistic.
- [x] **P2F-03d** Download/run MediaMTX locally (`rtmp://127.0.0.1/live/dev`), view with VLC/ffplay. No public keys.
- [x] **P2F-03e** Validate horizontal-only, vertical-only, and both. Capture per-target results and timings.
- [x] **P2F-03f** Confirm app does not store stream keys; it only uses OBS/Aitum/keychain configuration. (`P2-11` / `P2-45`).
- [x] **P2F-03g** One private/unlisted end-to-end stream + 30-minute soak; log drops/reconnects.
- [x] **P2F-03h** **Vertical audio:** record vertical-only and both; inspect files with `ffprobe` to confirm mic + screen audio are present. If absent, add the existing audio sources to every vertical scene (`CreateSceneItem` against the vertical scene UUID) and retest.
- [x] **P2F-03i** **Vertical encoder:** record which encoder Aitum's vertical output uses (vendor/settings/logs). Confirm no ~100% CPU regression from "Use main OBS settings" being off; document the safe setting.

**Acceptance:** three local stream cases pass; degraded state covered by tests; vertical recording/stream contains audio; encoder choice documented; 30-minute soak sampled with desktop-load losses documented.

**Validation note (2026-10-26):** final soak ran **31m 8s** (60 samples); both outputs stayed active with no reconnects or MediaMTX errors. Horizontal skipped frames were 2.6%; vertical skipped frames were 0%; render skipped grew 10.1%; active FPS ranged 37.1-60.0. This is a stability pass, not a zero-drop pass. Only local MediaMTX destinations were available; no public/unlisted stream key was used. A first attempt was invalidated by an explicit `StopStream` request and is excluded.

---

## Step 4 — Background removal (core)

- [ ] **P2F-04a** Manual test in OBS on a throwaway dev scene: does `background_removal` work outside the app? Determine plugin/install vs app-creation cause.
- [ ] **P2F-04b** Inspect OBS log (`Help → Log Files`) for model/runtime errors when the filter loads.
- [ ] **P2F-04c** Test with a bright background: if the whole camera rectangle shows through, the mask/model is empty (hypothesis: model failed to run).
- [ ] **P2F-04d** Install/use the plugin's official Windows installer if the current layout is wrong; verify model files.
- [x] **P2F-04e** Create the filter **once during bootstrap**, disabled; `toggle_background_removal` only calls `SetSourceFilterEnabled`.
- [x] **P2F-04f** Check `background_removal` filter kind during health check; expose `background_removal_available`; UI shows "Unavailable" instead of failing silently.
- [ ] **P2F-04g** Validate visually (clean edge, no white/transparent rectangle) and tick `P2-24`.

**Acceptance:** toggling works repeatedly, survives relayout/reconnect, and produces a correct cutout; missing plugin degrades to "Unavailable".

**Validation note (2026-10-26):** filter creation, repeated enable/disable, and the missing-filter "Unavailable" state are verified. A final visual cutout/edge-quality sign-off is still pending.

---

## Step 5 — Preview performance, graceful degradation, no-camera UX

- [x] **P2F-05a** Reduce screenshot width to ~640 px (16:9 640×360, 9:16 360×640); keep JPEG quality ~75.
- [x] **P2F-05b** Make preview FPS a setting (default 15, range 5–30) in `PersistedSettings` + Settings modal; backend reads it atomically.
- [x] **P2F-05c** Pause preview when the window is hidden (already wired via `visibilitychange`; verify) and when OBS is struggling: sample `GetStats` (~every 5 s), back off to ~2 FPS while `averageFrameRenderTime` is high or `activeFps` collapses; recover automatically.
- [x] **P2F-05d** Missing background image: on startup/sync, if the file is gone, set mode `none`, persist, emit a warning toast; UI stays usable.
- [x] **P2F-05e** No camera: camera slot shows a clear "Camera disconnected" placeholder (preview + live overlay); no black rectangle. Do not silently crop layouts.
- [ ] **P2F-05f** Verify `P2-38` small-window behavior (1120×720 minimum and below where possible).

**Acceptance:** preview honors the setting; no screenshot work while hidden; OBS render health recovers; missing files/camera never break the UI.

**Validation note (2026-10-26):** 640 px captures, 5-30 FPS persistence, hidden pause, health backoff/recovery, missing-background fallback, and camera placeholders are verified. Small-window (`P2F-05f`) remains open.

---

## Step 6 — Recording integrity

- [x] **P2F-06a** Attempt `SplitRecordFile` first and log the exact path taken (`native split` vs `close/reopen fallback`).
- [x] **P2F-06b** If fallback is unavoidable, quantify the gap with `ffprobe` durations/`ffmpeg` concat; document expected gap. Investigate whether Hybrid MP4 can split natively on OBS 32.2.2.
- [x] **P2F-06c** Surface a UI warning when the fallback is used.
- [x] **P2F-06d** Verify split files for 16:9, 9:16, and both; tick `P2-10` only if integrity is acceptable.

**Acceptance:** split path is logged; consecutive files are verified; no silent gaps.

**Validation note (2026-10-26):** native `SplitRecordFile` is rejected on OBS 32.2.2, so every tested split used the logged close/reopen fallback. The UI warning is verified. Measured 16:9 gap is 1.318-1.599 s; 9:16 handoff is about 45-70 ms. Both-canvas split now completes with four valid files, but it is not gapless.

---

## Step 7 — End-to-end matrix

- [ ] **P2F-07a** All 8 layouts × both canvases, with and without camera.
- [ ] **P2F-07b** Five rapid clicks within one second.
- [ ] **P2F-07c** Mic + screen-audio capture/monitor/volume, including meters.
- [ ] **P2F-07d** Quick resize (corners, size, circle/tile, split ratio) and preset save/load.
- [x] **P2F-07e** Record + split + inspect files; stream three ways.
- [ ] **P2F-07f** Kill OBS mid-session; confirm reconnect, state restore, and clear errors.
- [x] **P2F-07g** Confirm Phase 0/1 docs contain circle↔tile radius evidence and rapid-click evidence (they do; add cross-links if missing).

**Acceptance:** every matrix row passes; failures are documented with logs.

**Validation note (2026-10-26):** record+split+stream (`P2F-07e`) and the Phase 0/1 cross-links (`P2F-07g`) are complete. The remaining matrix rows are not part of this validation pass.

---

## Step 8 — Performance & health

- [ ] **P2F-08a** Use OBS `GetStats`: render time, skipped frames, active FPS; targets: 0 skipped frames, render time well under 16.6 ms.
- [ ] **P2F-08b** Measure idle/recording with preview on vs off; record CPU/RAM/startup and release binary size (`P2-37`).
- [ ] **P2F-08c** Drive the `Healthy` badge from connection + render stats, not just WebSocket state.

**Acceptance:** numbers recorded in `docs/phase2-results.md`; no skipped frames at 1080p60 in the test profile.

**Validation note (2026-10-26):** `GetStats` was sampled during the soak: active FPS 37.1-60.0 (average 54.0), render time 3.76-22.13 ms (average 10.56), horizontal skipped frames 2.6%, vertical skipped frames 0%, render skipped 10.1%. The 0-skipped-frame target was **not** met while the desktop was active; details are in the results document. `P2F-08a..c` remain open.

---

## Step 9 — UI polish

- [ ] **P2F-09a** Small-window responsiveness (`P2-38`) with the dock, preview, and right rail.
- [ ] **P2F-09b** Visual parity with the reference screenshot (`P2-36`) except deferred chat/stats.
- [x] **P2F-09c** First-run checklist: OBS version, WebSocket, Move, Aitum, Multi-RTMP, masks, Background Removal — red/green. (Settings → Plugins exists; verify and auto-surface once on first run.)

**Acceptance:** no clipped controls at minimum size; parity notes documented; first-run checklist shown when anything is missing.

**Validation note (2026-10-26):** first-run checklist auto-surface (`P2F-09c`) is verified. Small-window and visual-parity checks remain open.

---

## Step 10 — Release & docs

- [ ] **P2F-10a** `npm run tauri build` → NSIS/MSI; smoke-test on a clean Windows profile (`P2-40`); check WebView2 bootstrap.
- [ ] **P2F-10b** Consider code signing to avoid SmartScreen warnings (note if deferred).
- [x] **P2F-10c** Write `docs/phase2-results.md` with metrics, matrix results, stream/recording evidence, screenshots, and known limitations.
- [x] **P2F-10d** Tick `P2-*` boxes in `TASKS.md`, append progress-log rows, update README status.
- [ ] **P2F-10e** Final `P2-GATE` sign-off.

**Acceptance:** installer exists, clean-machine demo passes, docs updated, gate signed.

**Validation note (2026-10-26):** results document and tracker updates are complete. Installer/signing/clean-profile smoke test and the final user sign-off remain deferred.

---

## After Phase 2 (backlog, not now)

- Extra ratios (1:1, 4:5) as additional canvases.
- Destination manager UI with keychain storage.
- Disk/battery watchdog that splits recordings.
- Hotkeys, Stream Deck, phone remote page.
- Phase 4: chat, guests, extra destinations.

---

## Current known risks

| Risk | Mitigation |
|---|---|
| Aitum vertical output has no destination / different vendor API | Probe live vendor responses; keep UI error text actionable; test locally with MediaMTX |
| Vertical output audio missing | ffprobe vertical recordings; add audio sources to vertical scenes if needed |
| Vertical encoder pegs CPU | Read encoder from Aitum settings/logs; keep "use main OBS settings" unless proven safe |
| Hybrid MP4 split fallback gap | Log path; measure with ffprobe; warn in UI; investigate native split |
| Background removal plugin model broken | Manual test, OBS logs, official installer, bright-background diagnostic |
| Screenshot preview costs GPU/CPU | 640 px, configurable FPS, hidden-window pause, GetStats backoff |
| Dev state contaminates real config | `Vidyst_Dev` collection + `VIDYST_CONFIG_DIR=.dev-state` + reset/restore scripts |
