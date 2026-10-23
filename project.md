# Project: Vidyst-Style Studio Controller on Top of OBS Studio (V1 Core)

---

## 0. YOUR ROLE & OPERATING RULES (READ CAREFULLY)
You are an expert Rust, Tauri v2, Svelte 5, and OBS WebSocket v5 (`obws`) systems engineer. 
You are tasked with planning, scaffolding, testing, and building a fast, modern desktop studio application inspired by **Vidyst** (`vidyst.com`), built as a **lightweight companion controller on top of OBS Studio**.

### STRICT GUARDRAILS — DO NOT VIOLATE:
1. **DO NOT build a custom video compositor, audio clock, or encoder from scratch.** OBS Studio is the sole video/audio capture, compositing, recording, and streaming engine.
2. **DO NOT write a C/C++/Rust in-process OBS plugin for V1.** All communication with OBS must happen out-of-process over `obs-websocket` v5 using the Rust `obws` crate (v0.15+).
3. **DO NOT hand-write OBS Scene Collection JSON files.** Always create, inspect, and modify scenes, inputs, transforms, and filters programmatically through `obs-websocket` requests so OBS validates them live.
4. **DO NOT build Live Chat, Local Web Overlay Servers (`axum`), or Remote Guest/Podcast rooms in V1.** Those are strictly Phase 4 future features. Do not include them in V1 dependencies or code.
5. **DO NOT use `Fit::Cover` as the default for 16:9 Screens inside 9:16 Vertical layouts.** A 16:9 screen inside a vertical slot must default to `Fit::Contain` (`OBS_BOUNDS_SCALE_INNER`) so none of the horizontal screen content is cropped off.
6. **DO NOT use OBS Virtual Camera for the live studio preview.** Virtual Camera adds latency and only displays the main canvas. Use OBS's native zero-latency projector (`OpenVideoMixProjector`) or dock the controller UI alongside OBS.
7. **Always verify OBS existence, WebSocket connectivity, and installed plugins FIRST** before executing scene-generation code.

---

## 1. PRODUCT VISION & USER EXPERIENCE (WHAT WE ARE BUILDING)
The user wants the speed, visual polish, and 1-click animated layout workflow of **Vidyst**, while relying on **OBS Studio** underneath for rock-solid capture, hardware encoding, and streaming.

### Visual UI Layout (ASCII Reference of User's Screenshots)
The application window has a dark, modern studio interface structured as follows:

```text
+-----------------------------------------------------------------------------------+
| [● LIVE / RECORD]  Side by side inverted    [Horizontal 16:9] [Vertical 9:16] ... | <- Top Bar
+-----------------------------------------------------------------------------------+
|                                                                                   |
|                                                                                   |
|                                                                                   |
|                            LIVE VIDEO PREVIEW AREA                                |
|            (Docked next to OBS Preview or OBS Windowed Projector)                 |
|                                                                                   |
|   +--- Quick Controls Toolbar / Popover --------------------------------------+   |
|   | [Cam: Webcam/Mobile v] [Screen Picker v] [BG Remove: ON/OFF] [BG Image v] |   |
|   | [Mic: Mute | Listen | Vol]  [Screen Audio: Mute | Listen | Vol]           |   |
|   | [Quick Resize: Snap TL/TR/BL/BR | Cam Size Slider | Circle <-> Tile]      |   |
|   +---------------------------------------------------------------------------+   |
|                                                                                   |
+-----------------------------------------------------------------------------------+
| Layouts                                                                           | <- Bottom Dock
| SOLO          SCREEN                                                              |
| +----------+  +----------+ +----------+ +----------+ +----------+ +----------+    |
| |          |  |  Purple  | |  Purple  | |  Purple  | |  Purple  | |Purp |Brwn|    |
| |  Brown   |  |  Screen  | |  Screen  | |  Screen  | |  Screen  | |Scrn |Cam |    |
| |  Cam     |  |     (O)  | | (O)      | |    [##]  | | [##]     | | 75% |25% |    |
| +----------+  +----------+ +----------+ +----------+ +----------+ +----------+    |
|   Just me     Screen+cam   Screen+cam   Screen+tile  Screen+tile  Side by side    |
|                             inverted                  inverted    (+ inverted &   |
|                                                                    Screen focus)  |
+-----------------------------------------------------------------------------------+

```


### How the Bottom Layout Strip Works Visually:

- In the bottom "Layouts" dock, each layout thumbnail is drawn procedurally from normalized rectangle data (no static image assets needed):
    
    - **Purple box** = Screen capture slot.
        
    - **Brown box / circle** = Camera video slot (streamer face).
        
- When the user clicks any of the **8 layout thumbnails**:
    
    - OBS smoothly animates the Screen and Camera from their current positions/sizes to the target layout over **350ms (Ease-Out)** at 60fps.
        
    - If switching between a **Circle camera** (`Screen + cam`) and a **Rectangular Tile/Strip camera** (`Screen + tile` or `Side by side`), the camera's corner radius smoothly morphs between a circle (`50%` radius) and a rounded rectangle (`4%–8%` radius) while moving across the screen.
        

## 2. COMPLETE V1 CORE FEATURE CHECKLIST

You must implement these exact V1 features:

1. **1-Click Animated Layout Switching (8 Built-in Layouts):**
    
    - `just_me` (Solo group: Full-frame camera, screen hidden)
        
    - `screen_cam` (Screen group: Full screen + Circle camera at Bottom-Right)
        
    - `screen_cam_inv` (Screen group: Full screen + Circle camera at Bottom-Left)
        
    - `screen_tile` (Screen group: Full screen + Rounded Rectangular camera at Bottom-Right)
        
    - `screen_tile_inv` (Screen group: Full screen + Rounded Rectangular camera at Bottom-Left)
        
    - `side_by_side` (Screen group: Screen on Left ~73%, Camera strip on Right ~23%)
        
    - `side_by_side_inv` (Screen group: Camera strip on Left ~23%, Screen on Right ~73%)
        
    - `screen_focus` (Screen group: Full screen only, camera hidden)
        
2. **Multi-Aspect Ratio Support (Horizontal, Vertical, Both, + Custom Ratios):**
    
    - Top bar toggles for **`Horizontal (16:9)`** and **`Vertical (9:16)`** (multi-select: user can run Horizontal only, Vertical only, or Both simultaneously), plus extensible support for `1:1` and `4:5`.
        
    - Clicking a layout animates both the 16:9 canvas and the 9:16 vertical canvas (via Aitum Vertical linked scenes or vendor WebSocket requests).
        
3. **1–2 Click Live Streaming & Crash-Safe Recording:**
    
    - **Record Button:** Starts/stops recording using **Hybrid MP4** (`mp4` / `hybrid_mp4` or `mkv`) + automatic file splitting (`SplitRecordFile`) so 4K60 recordings are crash-resistant.
        
    - **Go Live Button:** Starts/stops streaming. Stream keys are stored in the OS keychain (`keyring` crate). Syncs with `Aitum Vertical` and `obs-multi-rtmp` so 1 click streams to all configured destinations.
        
4. **Multi-Device Video Switching (Webcam, Mobile Phone, Screen):**
    
    - Switch camera source between local USB Webcam, Mobile Phone camera (Continuity Camera on macOS, DroidCam/Camo virtual webcam, or VDO.Ninja browser source).
        
    - Switch screen capture target between full display or specific application windows.
        
    - Toggle camera or screen visibility on/off cleanly.
        
5. **Audio Capture + "Listen While Capturing" (External Mic & Screen Audio):**
    
    - Separate controls for **External Microphone** and **Screen / Desktop / App Audio**.
        
    - Each audio source has 3 controls:
        
        1. **Capture Toggle (Mute/Unmute):** Whether viewers/recordings hear the audio (`SetInputMute`).
            
        2. **Listen Toggle (Audio Monitoring):** Whether the creator hears the captured screen/app audio in their headphones while recording (`SetInputAudioMonitorType`: `OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT` vs `OBS_MONITORING_TYPE_NONE`).
            
        3. **Volume Slider:** Input level in dB (`SetInputVolume`).
            
6. **1-Click Background Removal & Custom Backgrounds:**
    
    - Toggle the `Background Removal` filter on the camera source (`SetSourceFilterEnabled`).
        
    - Swap or toggle a background image/color/blur layer directly behind the camera silhouette (`SetInputSettings` / `SetSceneItemEnabled`).
        
7. **Fast Live Resizing & Custom Layout Saving:**
    
    - Quick controls to snap camera to any corner (`TL`, `TR`, `BL`, `BR`), drag a **Camera Size Slider**, toggle **Circle vs. Tile**, or adjust the **Side-by-Side split ratio**.
        
    - Sends throttled (30–60 Hz) `SetSceneItemTransform` calls to OBS and lets the user save the tweak as a reusable layout preset in JSON.
  
