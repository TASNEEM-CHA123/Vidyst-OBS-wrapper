# Phase 2 validation results

- Generated: **2026-10-26**
- OBS Studio: **32.2.2**; obs-websocket: **5.7.4**
- App: Tauri 2 + Svelte 5 (`Vidyst_Dev` scene collection, `VIDYST_CONFIG_DIR=.dev-state`)
- Overall: **V1 demo verified locally; release installer and final user sign-off remain**
- Detailed checklist: [`TASKS-PHASE2-FINISH.md`](../TASKS-PHASE2-FINISH.md)
- Earlier evidence: [`phase0-results.md`](./phase0-results.md), [`phase1-results.md`](./phase1-results.md)

## Summary

| Area | Result | Evidence |
|---|---|---|
| Isolated dev state | Pass | `Vidyst_Dev` collection, `.dev-state`, `.dev-recordings`, `devctl setup/reset/restore/status/probe/raw` |
| Streaming state machine | Pass | `Idle -> Starting -> Live -> Stopping -> Error`, plus `Degraded`; 19 workspace tests, including 8 stream-state tests |
| 16:9 + 9:16 local streaming | Pass | Both outputs live to MediaMTX; H.264/AAC on both |
| Partial-failure recovery | Implemented/tested | Degraded-state unit tests and retry path; live output-loss injection not part of this run |
| Vertical audio/encoder | Pass | AAC 48 kHz stereo; QSV H.264 CBR 15 Mbps, 1080x1920 |
| 30-minute soak | Completed with caveat | Both outputs stayed live for 31m; 2.6% horizontal video frames skipped and 10.1% render frames skipped under desktop load; no disconnects |
| Split recording | Fallback only | Native `SplitRecordFile` rejected by OBS 32.2.2; close/reopen fallback warns and logs |
| Background removal | Functional | Filter created once, toggled only; missing filter shows "Unavailable"; visual cutout sign-off still pending |
| Missing background image | Pass | Falls back to `none`, clears path, persists, shows warning |
| Camera disconnect | Pass | "Camera disconnected" overlays; reconnect restores "Camera active." |
| Preview performance | Pass with caveat | 640 px, 15 FPS default, 5-30 range, hidden pause, health backoff/recovery |
| First-run/plugin checklist | Pass | OBS, WebSocket, 2 canvases, all five plugins green; auto-surfaces once |
| Automated checks | Pass | `cargo fmt`, `cargo check`, 19 Rust tests (8 stream-state), Svelte check/build |

## Environment and isolation

- Real scene collection: backed up before work; restored after validation with `devctl restore --state .dev-state/original.json`.
- Development collection: `Vidyst_Dev`; test state lives in `.dev-state/` and `.dev-recordings/`, both gitignored.
- Test destinations were local-only:
  - Horizontal: `rtmp://127.0.0.1/live`, local test key, MediaMTX on port **1935**.
  - Vertical: `rtmp://127.0.0.1:1936/live`, local test key, MediaMTX on port **1936**.
- No real stream key, OBS password, or `.env` was committed. The app stores `stream_server = null`; OBS/Aitum hold destinations.

## Streaming

### State machine and failure handling

The controller keeps an explicit stream state and reconciles it from the two independent outlets:

- `Idle -> Starting -> Live -> Stopping -> Error`
- A failed second leg while the first is live becomes **`Degraded`**, not an all-or-nothing stop.
- Stop actions are sent to both legs concurrently; a failed stop stays in a degraded stop state until the outputs actually end.
- Retry starts only the failed target.
- The UI never shows `Live` optimistically.

`cargo test -p vidyst-studio` covers eight state-machine cases:

```text
degraded_outputs_recovering_returns_to_live ... ok
live_output_lost_degrades_and_offers_retry ... ok
error_with_one_surviving_output_becomes_degraded ... ok
idle_adopts_an_output_started_outside_the_controller ... ok
starting_waits_for_every_requested_output ... ok
failed_stop_stays_degraded_even_while_outputs_report_live ... ok
stopping_stays_busy_until_the_last_output_ends ... ok
losing_every_output_returns_to_idle ... ok
```

A live partial-failure injection (for example, killing only the vertical MediaMTX listener while horizontal remains live) was not repeated in this validation run. The degraded transition and retry behaviour are covered by the unit tests above.

### Local streaming cases

| Case | Horizontal | Vertical | Result |
|---|---|---|---|
| 16:9 only | 2560x1440 H.264 + AAC 48 kHz stereo | off | Pass |
| 9:16 only | off | 1080x1920 H.264 + AAC 48 kHz stereo | Pass |
| Both | 2560x1440 H.264 + AAC 48 kHz stereo | 1080x1920 H.264 + AAC 48 kHz stereo | Pass |

Both MediaMTX relays received active output. The stream start/stop UI reached `LIVE` only after OBS reported the requested outputs active, and returned to `IDLE` after both outputs stopped.

### Vertical encoder

From the OBS log while the vertical recording was active:

```text
[qsv encoder: 'vertical_canvas_video_encoder'] settings:
  codec:          H.264
  rate_control:   CBR
  target_bitrate: 15000
  width:          1080
  height:         1920
[FFmpeg aac encoder: 'Track1_vertical'] bitrate: 160, channels: 2, channel_layout: stereo
```

Aitum's vertical configuration uses `recording_match_main=false`, `record_encoder=""`, and `audio_bitrate=160`; the observed encoder was Intel QSV H.264. Vertical stream/record files contained AAC 48 kHz stereo audio.

### Stalled-output detection

The app reconciles stream health from OBS `outputActive` flags. A byte-growth stall detector (marking an output degraded when `outputBytes`/`outputDuration` stop moving) is **not implemented**. The 30-minute soak below samples byte and duration growth so this limitation is visible rather than hidden.

### 30-minute soak

Method:

- 60 samples at 30-second intervals (about 30-35 minutes of wall time including request overhead).
- The Tauri window was minimized so the screenshot preview was paused; both OBS outputs stayed live.
- Each sample read `GetOutputStatus` for `adv_stream` and the Aitum vertical output, `GetStreamStatus`, Aitum's vendor `status`, and OBS `GetStats`; MediaMTX logs supplied connection counts/errors.
- The sampler aborts only after three consecutive samples show both outlets inactive. A byte/duration plateau while `outputActive=true` is recorded, not auto-recovered.

**Result: completed.** The final run sampled from `13:52:53` to `14:24:02` (60 samples, **31m 8s**). Both outputs were active in every sample; there were no stream reconnects and no MediaMTX errors.

| Metric | 16:9 (`adv_stream`) | 9:16 (Aitum) |
|---|---:|---:|
| Output bytes | 134.1 MB -> 3,347.3 MB (+3.213 GB) | 146.3 MB -> 3,280.4 MB (+3.134 GB) |
| OBS output duration | 79.1 s -> 1,810.4 s | 78.1 s -> 1,730.3 s |
| Reported duration covered | 28m 51s | 27m 32s |
| Average bitrate over sample window | 13.76 Mbps | 13.42 Mbps |
| Skipped/total output frames | 2,859 / 108,626 (**2.6%**) | 0 / 103,819 (**0%**) |
| MediaMTX connections | 8 opened / 7 closed / 0 errors | 8 opened / 7 closed / 0 errors |

- Active FPS range: **37.1-60.0**, average **54.0**.
- Render time range: **3.76-22.13 ms**, average **10.56 ms**.
- Render frames skipped during the sample window: **11,366 / 112,021 (10.1%)**.
- Horizontal congestion spikes were observed at `13:54:32` (0.81) and `14:16:44` (1.0), but neither caused a reconnect or an inactive output.
- OBS working set ranged from about **144 MB to 477 MB**; the Tauri app was running and minimized for all samples.
- Output duration advanced more slowly than wall time (28m51s and 27m32s vs 31m8s), which is consistent with rendering/encoding lag; bytes and duration kept growing and neither output stopped.

This is **not** a zero-drop pass. The machine could not sustain 2560x1440 and 1080x1920 at 60 fps with zero skipped frames while the desktop was active. It is a stability pass: no disconnects, no MediaMTX errors, and no stalled byte/duration growth.

**Invalidated first attempt:** the `13:35` sampler run was discarded. At `13:38:02` and `13:38:08` the two outputs stopped after an explicit `StopStream` request; OBS logged `User stopped the stream`. There was no network/reconnect failure. The final run above is the evidence used.

## Recording and split behaviour

### Native split is unavailable on this OBS build

`SplitRecordFile` returned:

```text
API error: RequestProcessingFailed
```

The result was reproduced with the default Hybrid MP4 recording and with the app's MKV selection. OBS still reported the output as an `mp4 output` in the log, so the MKV selection should not be treated as a true MKV muxer validation until OBS is restarted after changing formats.

The controller therefore uses a bounded close/reopen fallback. It logs the path taken and returns a warning toast; it does not claim the split was seamless.

```text
recording split: 16:9 native split failed (API error: RequestProcessingFailed); using close/reopen fallback
recording split: horizontal=close/reopen fallback vertical=close/reopen fallback gapless=false
```

### 16:9 only

- First file: `8.966667 s`
- Second file: `40.816667 s`
- OBS stopped at `13:11:00.592`; the next recording started at `13:11:01.910`
- Measured fallback gap: **1.318 s**
- UI warning shown; path logged as `close/reopen fallback`

### 9:16 only

- First file: `8.783333 s`, `1080x1920`, H.264/AAC 48 kHz stereo
- Second file: `6.050000 s`, `1080x1920`, H.264/AAC 48 kHz stereo
- File handoff in the OBS log was about `45 ms` (`13:18:35.745` finalize -> `13:18:35.790` next writer)
- Encoded duration was shorter than wall time because OBS was dropping frames under desktop load
- UI warning: `Recording split; 9:16 restarted and may contain a short gap.`

### Both canvases

The both-canvas fallback now completes after allowing Aitum up to 15 seconds to report its stop/start state:

| File | Duration | Video | Audio |
|---|---:|---|---|
| `2026-10-26 13-32-10.mkv` (16:9) | 8.200 s | H.264 2560x1440 | AAC 48 kHz stereo |
| `2026-10-26 13-32-14-vertical.mkv` (9:16) | 12.400 s | H.264 1080x1920 | AAC 48 kHz stereo |
| `2026-10-26 13-32-25.mkv` (16:9) | 10.550 s | H.264 2560x1440 | AAC 48 kHz stereo |
| `2026-10-26 13-32-32-vertical.mkv` (9:16) | 2.817 s | H.264 1080x1920 | AAC 48 kHz stereo |

- Horizontal stop `13:32:25.337` -> restart `13:32:26.936`: **1.599 s** fallback gap.
- Vertical handoff `13:32:32.590` -> next writer `13:32:32.660`: about **70 ms**.
- The files were valid and all contained stereo AAC audio. The split is **not gapless**; the UI warning and `gapless=false` log make that explicit.

### Recording integrity limitation

The "gapless where possible" requirement cannot be met for this output type on OBS 32.2.2: native split is rejected even after the fallback path was isolated and bounded. The implemented contract is therefore:

1. Try native split first.
2. Log the exact path.
3. Fall back to close/reopen only when necessary.
4. Return a report with `gapless=false` and a user-visible warning.
5. Bound every stop/start request so the UI can recover instead of hanging.

## Background removal

- The `background_removal` filter is created once during bootstrap and left disabled.
- Toggling calls only `SetSourceFilterEnabled`.
- `filterEnabled=true/false` was observed in OBS after UI toggles.
- Removing the filter caused the UI to show **Unavailable** and disable the checkbox; recreating it restored availability.
- A final visual edge-quality/cutout sign-off (clean edge, no white/transparent rectangle) is still pending. The functional path and missing-plugin degradation are verified.

## Graceful degradation

### Missing background image

A saved background image was removed from disk before startup. The app:

- changed `background_mode` to `none`
- cleared the persisted image path
- kept the UI usable
- showed: `The saved background image is missing. Background layer turned off; choose another image.`
- emitted the `background_missing` event

No black or broken source was created.

### Camera disconnected

With the camera disconnected:

- both preview and live canvases displayed "Camera disconnected" overlays
- the layout geometry did not change and the camera was not silently cropped
- a toast reported: `Camera disconnected and released.`
- reconnecting restored "Camera active."

## Preview performance

- Preview captures are JPEG `GetSourceScreenshot` frames at approximately 640 px wide:
  - 16:9: 640x360
  - 9:16: 360x640
- Preview FPS is persisted with default **15** and clamped to **5-30**; a 10 FPS value was written to `.dev-state/settings.json` and restored to 15 after the test.
- The preview pauses when the window is hidden and when OBS render health degrades.
- Health is sampled about every 5 seconds. Observed backoff and recovery:

```text
preview health: active_fps=0.4 render_ms=2383.2 struggling=true
preview health: active_fps=38.7 render_ms=7.2 struggling=false
```

- While the app was minimized during the soak, preview work was paused; this is the intended behavior.
- Known limitation: WebView timer throttling while hidden can delay snapshot updates after an external OBS change until the UI is focused or refreshed.

## First-run and plugin health

With WebView local storage cleared and the app stopped:

1. The app auto-opened Settings on the Plugins tab.
2. It ran the health check.
3. It reported `Plugin set ready.`

The checklist verified:

- OBS 32.2.2
- obs-websocket 5.7.4
- 2 canvases
- Move Transition
- Advanced Masks
- Aitum Vertical Canvas
- Background Removal
- Multi RTMP

The first-run key `vidyst.firstRunChecklist` is set in WebView local storage after a successful run and auto-surfaces again whenever scene setup is incomplete.

## Automated checks

Run on the final source tree:

```powershell
cargo fmt --all
cargo check --workspace --all-targets
cargo test --workspace
npm --prefix ui run check
npm --prefix ui run build
```

Results:

- `cargo fmt`: clean
- `cargo check --workspace --all-targets`: pass
- `cargo test --workspace`: **19 passed** (9 layout, 2 driver, 8 studio stream-state)
- `cargo test -p vidyst-studio`: **8 passed**
- `npm --prefix ui run check`: **0 errors, 0 warnings**
- `npm --prefix ui run build`: pass

## Known limitations and deferred work

1. **Stalled output detection** is not implemented; stream state follows `outputActive`. A byte/duration growth monitor is the next reliability improvement.
2. **No public/unlisted RTMP service** was tested because no real destination keys were stored. MediaMTX local relays are the private-stream equivalent used here.
3. **Native split is rejected** by OBS 32.2.2 for this output; the fallback introduces a measured gap (1.3-1.6 s horizontal). Both-canvas split completes but is not gapless.
4. **Vertical under simultaneous encoding** is environment-limited. The machine showed up to ~95% encoding lag when recording both canvases plus screenshot preview; the longer Aitum wait makes the split complete, but the gap remains.
5. **Background removal visual quality** still needs a final cutout sign-off.
6. **Full end-to-end matrix** (all 8 layouts with/without camera), **small-window polish**, **Healthy badge from render stats**, and **release installer/clean-profile smoke test** remain outstanding.
7. The desktop was actively used during some tests; OBS reported skipped frames. The 0-skipped-frame performance target was not met in this environment.

## Restore and cleanup

After the soak:

- `.dev-state/settings.json` was reset to `record_format=hybrid_mp4`, `preview_fps=15`, `background_mode=none`, `background_image=null`.
- OBS was returned to the real collection: **Untitled**, scene **Vidyst_side_by_side**, transition **Move 350 ms**, vertical scene **Vidyst_side_by_side**; mic and screen-audio volumes/mute/monitor state match the saved original.
- OBS stream service destination was cleared: `rtmp_custom`, server `rtmp://`, empty key. Record directory returned to `C:\Users\HI-TECH\Videos`.
- Aitum's original `config.json` was restored from the 1,384-byte backup; it contains no `127.0.0.1` destination and no `.dev-recordings` path.
- `.dev-recordings` was emptied; the real `Videos` folder had no test files. OBS, the Tauri app, and both MediaMTX processes were stopped; ports 1420/1935/1936/4455 are closed.
- During the first attempt to switch OBS back to the real collection, OBS 32.2.2 crashed in `obs.dll` (Application Error `0xc0000005`). OBS was restarted normally, loaded the real collection, and the reset/verification above completed. Treat collection switching with this plugin set as a fragile OBS operation; starting OBS directly on the target collection is safer than switching while running.

## Reproduction notes

- Use `scripts/dev.ps1` to start OBS in the isolated `Vidyst_Dev` collection and Tauri dev with `VIDYST_CONFIG_DIR=.dev-state`.
- Use `scripts/dev-reset.ps1` to reset dev state; use `devctl restore --state .dev-state/original.json` to return to the real collection.
- Use `devctl probe --request-type status` for Aitum recording/streaming state.
- Do not commit `.dev-state/`, `.dev-recordings/`, `backups/`, or `.env`.
