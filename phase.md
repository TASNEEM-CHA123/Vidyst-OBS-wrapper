## 5. STEP-BY-STEP EXECUTION PLAN FOR THE AGENT

Follow these phases in strict sequential order. **Do not jump to Phase 2 until Phase 0 and Phase 1 are verified.**

### PHASE 0: Environment Discovery, OBS Connection & Plugin Verification

Before building any UI, use your terminal/shell tools to inspect the user's machine and run a Rust connection spike:

1. **Detect OS and Locate OBS Binary:**
    
    - **macOS:** Check `/Applications/OBS.app/Contents/MacOS/obs` and plugin directory `~/Library/Application Support/obs-studio/plugins/`.
        
    - **Windows:** Check `C:\Program Files\obs-studio\bin\64bit\obs64.exe` and `C:\ProgramData\obs-studio\plugins\`.
        
    - **Linux:** Check `which obs` and `~/.config/obs-studio/plugins/`.
        
2. **Connect to `obs-websocket` (Port `4455`):**
    
    - If OBS is not running, launch it or ask the user to open OBS Studio with `Tools -> WebSocket Server Settings -> Enable WebSocket Server` (Port `4455`, either no auth on localhost or read password from user/env).
        
    - Write a small Rust CLI spike using `obws::Client::connect("127.0.0.1", 4455, password).await` and print:
        
        - OBS Studio version & `obs-websocket` version (`client.general().version().await`).
            
        - Available input kinds (`client.inputs().list_kinds(false).await`).
            
        - Available filter kinds (`client.filters().list_kinds().await`) -> Check for `move_value_filter` / `move_source_filter` (`obs-move-transition`), `mask_filter` (`obs-advanced-masks`), and `background_removal` (`obs-backgroundremoval`).
            
        - Available transition kinds -> Check for `move_transition`.
            
    - If any of the 4 plugins are missing, print exact download/install instructions for the user's OS.
        

### PHASE 1: The 5-Test Validation Spike (`obs-driver` CLI Test)

Before creating the Tauri/Svelte frontend, write an automated test binary in `crates/obs-driver/src/bin/spike.rs` that validates:

1. **Programmatic Scene & Filter Bootstrap:**
    
    - Creates a wrapper scene `"Vidyst_Camera_Group"` containing the raw `"Vidyst_Camera_Raw"` input (so filters apply to the group cleanly).
        
    - Attaches `obs-advanced-masks` (Rectangle mask with Corner Radius %) and `Move Value` filters (`Morph_To_Circle` = 50% radius over 350ms, `Morph_To_Tile` = 8% radius over 350ms, triggered on filter enable).
        
    - Creates the 8 layout scenes (`Vidyst_just_me`, `Vidyst_screen_cam`, etc.) using `client.scenes().create(...)`.
        
    - Adds `"Vidyst_Screen"` and `"Vidyst_Camera_Group"` to each scene, sets `SetSceneItemTransform` using the `Slot` math above, and sets `SetSceneItemEnabled(false)` for hidden slots (e.g., Screen in `just_me`, Camera in `screen_focus`).
        
    - Sets OBS's current scene transition to `"Move"` with duration `350ms`.
        
2. **Test 1 — Smooth Circle <-> Tile Morph:** Switch from `Vidyst_screen_cam` (circle) to `Vidyst_side_by_side` (tile) while enabling `Morph_To_Tile`. Confirm smooth 350ms movement + corner radius animation.
    
3. **Test 2 — Rapid Click Test:** Switch across 5 layout scenes within 1 second (`tokio::time::sleep(Duration::from_millis(200))`) and verify Move Transition interpolates mid-flight without glitching. _(Fallback: If scene-per-layout glitches on rapid clicks, switch `obs-driver` to the Single Master Scene mode using `Move Source` filters on one scene)._
    
4. **Test 3 — Audio Mute & Live Listen Toggles:** Test `set_muted` and `set_audio_monitor_type(MonitorType::MonitorAndOutput)` on Mic and Screen Audio inputs.
    
5. **Test 4 — Vertical Canvas Sync:** Test switching the vertical scene via Aitum Vertical's Linked Scenes or `call_vendor_request("aitum-vertical-canvas", "switch_scene", ...)`.
    

### PHASE 2: Build the Tauri v2 + Svelte 5 Desktop Studio UI

Once the Rust `obs-driver` passes Phase 1:

1. **Top Bar (`TopBar.svelte`):**
    
    - Status indicator (`● LIVE` / `● REC` / `IDLE`).
        
    - Active layout name label (e.g., `Side by side inverted`).
        
    - Aspect Ratio multi-select buttons: `[Horizontal]` and `[Vertical]` (plus dropdown for `1:1` / `4:5`).
        
    - Primary action buttons: **`Record`** (Hybrid MP4 + `SplitRecordFile`) and **`Go Live`**.
        
2. **Bottom Layout Dock (`LayoutStrip.svelte`):**
    
    - Renders the 8 layouts grouped under `SOLO` and `SCREEN` headers.
        
    - Each card renders a miniature `16:9` (or `9:16` when previewing vertical) CSS/SVG box using the exact `Slot` rects from `crates/layouts`:
        
        - Screen `Slot` rendered in muted purple (`#4b4863`), Camera `Slot` rendered in warm brown (`#6b5b4b`) with `border-radius` matching `slot.radius_pct`[cite: 1, 2].
            
    - Clicking a card calls `invoke("apply_layout", { id: layout.id })`.
        
3. **Device, Background & Audio Controls (`AudioMixer.svelte` & `QuickCustomize.svelte`):**
    
    - **Video Devices:** Dropdown to select Webcam or Phone Camera; dropdown to select Screen/Window.
        
    - **Background:** Toggle button for `Remove Background` + picker for Background Image/Color behind the camera.
        
    - **Audio Mixer:** Two clean rows (`External Mic` and `Screen Audio`), each with `[Mute/Capture Toggle]`, `[Listen/Monitor Toggle]`, and `[Volume Slider]`.
        
    - **Quick Resize:** Corner snap buttons (`TL`, `TR`, `BL`, `BR`), `Cam Size` slider, and `Circle / Tile` toggle that call `invoke("quick_resize_camera", ...)` throttled to 30ms (`~33fps`) and a `Save Layout` button.
        

## 6. YOUR FIRST ACTION RIGHT NOW

1. Acknowledge this specification and confirm you understand the V1 boundaries (no custom C++ plugin, no hand-edited scene JSON, no chat/guests in V1, `Contain` for vertical screens).
    
2. Immediately begin **Phase 0**: Use your shell/file tools to check the current OS, detect whether OBS Studio is installed and running, check if port `4455` (`obs-websocket`) is reachable, and scaffold the Cargo workspace (`crates/layouts` and `crates/obs-driver`) to run the Phase 0 discovery check.
    

```

---

### How to Use This Prompt Effectively with Gemini 3.8 Flash
1. **Start with a clean workspace folder** (e.g., `mkdir vidyst-obs && cd vidyst-obs`).
2. **Open OBS Studio** on your machine and make sure **Tools $\rightarrow$ WebSocket Server Settings $\rightarrow$ Enable WebSocket Server** is checked.
3. **Paste the entire Markdown block above** into your Gemini 3.8 Flash coding agent. It is instructed to immediately inspect your OS, locate your OBS installation, test the port `4455` connection, check your installed plugins, and scaffold the Rust workspace step by step.
```