# Phase 0 Results — Environment, OBS & Plugin Verification

**Date:** 2026-10-22  
**Result:** ✅ **PASS** — OBS is reachable, all five required third-party plugins are loaded, the Rust workspace builds, and all Phase 0 tests pass.

Machine-readable report: [`phase0-results.json`](./phase0-results.json)

---

## 1. Workstation

| Item | Value |
|---|---|
| OS | Windows 11 Home Single Language, version 26H2 (10.0.26300) |
| CPU | 11th Gen Intel Core i5-1135G7 (4 cores / 8 threads) |
| RAM | 15.8 GB |
| GPU | Intel Iris Xe Graphics, driver 31.0.101.4502 |
| Free disk | 42.5 GB on `E:` |
| Project directory | `E:\Reseach Projects\vidyst-obs` |
| Rust | 1.99.1 MSVC |
| Node / npm | 24.3.0 / 11.4.2 |

## 2. OBS and WebSocket

| Item | Value |
|---|---|
| OBS Studio | **32.2.2** (requirement: 31.1+) |
| obs-websocket | **5.7.4** (built in) |
| RPC version | 1 |
| Endpoint | `127.0.0.1:4455` |
| Auth | Enabled; password is read from the OBS WebSocket config at runtime |
| OBS binary | `C:\Program Files\obs-studio\bin\64bit\obs64.exe` |
| Plugin directory | `C:\ProgramData\obs-studio\plugins` |
| Scene collection | `Untitled.json` (`Scene`), backed up before any mutation |

The password is never printed or committed. The discovery report records only where the credential came from (`ObsConfig`).

## 3. Required plugins

| Plugin | Version | Verification evidence |
|---|---|---|
| Move Transition | 3.2.1 | `move_source_filter` / `move_value_filter` / `move_transition` kinds; DLL loaded |
| Advanced Masks | 1.6.0 (legacy x64) | `advanced_masks_filter` and `advanced_masks_filter_v2` kinds; DLL loaded |
| Aitum Vertical Canvas | 1.6.6 | Vendor `version` request; `Aitum Vertical` canvas at 1080x1920; DLL loaded |
| Background Removal | 1.4.1 | `background_removal` filter kind; DLL loaded |
| Multi RTMP | 0.7.4.4 | System plugin files present; DLL loaded in OBS log |

> **Spec correction:** OBS's built-in `mask_filter` / `mask_filter_v2` is **not** evidence of Advanced Masks. The plugin exposes `advanced_masks_filter` / `advanced_masks_filter_v2`. Discovery was corrected accordingly after this false positive was found.

## 4. Canvases and transitions

- Canvases reported by OBS:
  - `Main` — 1920x1080
  - `Aitum Vertical` — 1080x1920
- Transition kinds: 8, including `move_transition`.
- Transition instances: `Cut`, `Fade`, `Swipe`, `Move`.
- A `Move` transition instance was added manually in OBS because obs-websocket and the OBS frontend API expose **no request to create a transition instance**. This is a one-time OBS setup requirement and is documented in the README. Discovery warns when the instance is missing.
- The `Move` transition is currently selected. Bootstrap will set its duration to 350 ms.

## 5. Automated checks

```powershell
cargo test --workspace
cargo run -p vidyst-obs-driver --bin phase0
cargo run -p vidyst-obs-driver --bin phase0 -- --json
```

Observed results:

- `cargo test --workspace`: **11 passed, 0 failed**
  - 9 layout geometry tests
  - 2 discovery tests
- `phase0`: exits **0**, reports `ready: true`, no missing plugins, no warnings.
- JSON report saved to [`phase0-results.json`](./phase0-results.json).

## 6. Backups made before Phase 1

Created in `backups/`:

- `Untitled-20261022-032942.json` — OBS scene collection before any Vidyst scene changes.
- `obs-websocket-config-20261022-032942.json` — WebSocket configuration.

## 7. What Phase 0 did not cover

Phase 0 deliberately does **not** create scenes, sources, filters, or transforms. Those are Phase 1 work. The Tauri/Svelte UI is intentionally not started until the Phase 1 validation gate passes.
