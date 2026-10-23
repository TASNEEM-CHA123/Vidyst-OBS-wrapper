      

## 3. FINALIZED TECH STACK & WORKSPACE STRUCTURE

### Tech Stack

- **App Shell:** **Tauri v2** (Rust backend + native OS webview; ~5 MB binary, near-zero idle CPU/RAM).
    
- **Async Runtime:** **`tokio`** (single persistent WebSocket connection to OBS, event subscriptions, throttled resize commands).
    
- **OBS WebSocket Client:** **`obws` crate (v0.15+)** (typed Rust client for `obs-websocket` v5 protocol).
    
- **Secret Management:** **`keyring` crate** (stores stream keys in macOS Keychain / Windows Credential Manager / Linux Secret Service).
    
- **UI Frontend:** **Svelte 5** (compiled, zero virtual-DOM, renders procedural SVG/CSS layout thumbnails, toggles, and sliders).
    
- **Required OBS Studio Setup (OBS 31.1+ / 32.x):**
    
    1. Built-in `obs-websocket` v5 enabled (default port `4455`).
        
    2. **`obs-move-transition` plugin** (by Exeldro) — for 60fps scene item and filter value animations.
        
    3. **`obs-advanced-masks` plugin** (by Exeldro) — for rounded-rectangle / circle camera masking.
        
    4. **`Aitum Vertical` plugin** — for 9:16 vertical canvas, vertical recording/streaming, and linked scenes.
        
    5. **`obs-backgroundremoval` plugin** & **`obs-multi-rtmp` plugin**.
        

### Cargo Workspace Layout

Plaintext

```
vidyst-obs-studio/
├── Cargo.toml                # Workspace root (lto = "fat", codegen-units = 1, strip = true)
├── crates/
│   ├── layouts/              # Pure Rust crate: Slot, Layout, AspectRatio, normalized-to-pixel math (no OBS dep)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── obs-driver/           # Rust crate wrapping `obws`: discovery, scene bootstrapper, layout switcher, audio/video toggles
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── discovery.rs  # Finds OBS binary, checks port 4455, verifies installed plugins
│           ├── bootstrap.rs  # Programmatically creates inputs, filters, and the 8 scenes via WebSocket
│           ├── switcher.rs   # Triggers scene switch + Move Value corner radius morph + vertical sync
│           └── controls.rs   # Audio mute/monitor, BG removal toggle, quick-resize transform updates
├── src-tauri/                # Tauri v2 binary crate wiring Svelte UI <-> obs-driver & layouts
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/main.rs
└── ui/                       # Svelte 5 frontend
    ├── package.json
    └── src/
        ├── App.svelte
        ├── components/
        │   ├── TopBar.svelte         # Go Live, Record, [Horizontal] [Vertical] toggles
        │   ├── LayoutStrip.svelte    # Procedural SVG/CSS thumbnails for the 8 layouts
        │   ├── AudioMixer.svelte     # Mic & Screen Audio: Mute, Listen (Monitor), Volume
        │   └── QuickCustomize.svelte # Device pickers, BG toggle, Corner snaps, Size slider
```

## 4. CORE DATA STRUCTURES & EXACT OBS TRANSFORM MATH (`crates/layouts`)

### Rust Data Structures

Rust

```
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AspectRatio {
    R16x9, // e.g., 1920x1080 or 3840x2160
    R9x16, // e.g., 1080x1920
    R1x1,  // e.g., 1080x1080
    R4x5,  // e.g., 1080x1350
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutGroup {
    Solo,
    Screen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceId {
    Screen,
    Camera,
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fit {
    Cover,   // Maps to OBS_BOUNDS_SCALE_OUTER + crop_to_bounds: true
    Contain, // Maps to OBS_BOUNDS_SCALE_INNER + crop_to_bounds: false
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32, // Normalized 0.0..1.0
    pub y: f32, // Normalized 0.0..1.0
    pub w: f32, // Normalized 0.0..1.0
    pub h: f32, // Normalized 0.0..1.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slot {
    pub source: SourceId,
    pub rect: Rect,
    pub radius_pct: f32, // 0.0 = sharp rect, 8.0 = rounded tile, 50.0 = circle
    pub fit: Fit,
    pub z: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub id: String,
    pub name: String,
    pub group: LayoutGroup,
    pub slots: BTreeMap<AspectRatio, Vec<Slot>>,
}
```

### Converting a `Slot` to an OBS `SceneItemTransform`

For a canvas of pixel dimensions `(canvas_w, canvas_h)`:

- `position_x = slot.rect.x * canvas_w`
    
- `position_y = slot.rect.y * canvas_h`
    
- `bounds_width = slot.rect.w * canvas_w`
    
- `bounds_height = slot.rect.h * canvas_h`
    
- `alignment = Alignment::TopLeft` (Value `5` in OBS alignment bitmask: `OBS_ALIGN_TOP | OBS_ALIGN_LEFT`)
    
- `bounds_alignment = Alignment::Center` (Value `0` in OBS)
    
- `bounds_type`:
    
    - If `slot.fit == Fit::Cover`: use `BoundsType::ScaleToOuterBounds` (`OBS_BOUNDS_SCALE_OUTER`) AND set `crop_to_bounds = true`.
        
    - If `slot.fit == Fit::Contain`: use `BoundsType::ScaleToInnerBounds` (`OBS_BOUNDS_SCALE_INNER`) AND set `crop_to_bounds = false`.
        

> **CRITICAL CIRCLE GEOMETRY NOTE:**
> 
> On a 16:9 canvas (`1920x1080`), for a Camera slot to render as a perfect 1:1 circle when `radius_pct = 50.0`, its pixel width (`w * 1920`) must equal its pixel height (`h * 1080`). Therefore `w = 0.140625` (`270px`) and `h = 0.25` (`270px`) produces an exact `270x270` square bounding box! On a 9:16 canvas (`1080x1920`), `w = 0.32` (`345.6px`) and `h = 0.18` (`345.6px`) produces an exact square bounding box.

### Exact Coordinates for the 8 Built-In Layouts

|**Layout ID**|**Display Name PNG+ 1**|**Group PNG+ 1**|**R16x9 (16:9 Horizontal) Slots**|**R9x16 (9:16 Vertical) Slots (Screen ALWAYS Contain)**|
|---|---|---|---|---|
|`just_me`|**Just me**|`Solo`|**Cam:** `x:0.02, y:0.03, w:0.96, h:0.94`, `r:4%`, `Cover`|**Cam:** `x:0.03, y:0.02, w:0.94, h:0.96`, `r:4%`, `Cover`|
|`screen_cam`|**Screen + cam**|`Screen`|**Screen:** `x:0.02, y:0.03, w:0.96, h:0.94`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Circle BR):** `x:0.83, y:0.70, w:0.1406, h:0.25`, `r:50%`, `Cover`|**Screen:** `x:0.02, y:0.08, w:0.96, h:0.50`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Circle BR):** `x:0.62, y:0.62, w:0.32, h:0.18`, `r:50%`, `Cover`|
|`screen_cam_inv`|**Screen + cam inverted**|`Screen`|**Screen:** `x:0.02, y:0.03, w:0.96, h:0.94`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Circle BL):** `x:0.03, y:0.70, w:0.1406, h:0.25`, `r:50%`, `Cover`|**Screen:** `x:0.02, y:0.08, w:0.96, h:0.50`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Circle BL):** `x:0.06, y:0.62, w:0.32, h:0.18`, `r:50%`, `Cover`|
|`screen_tile`|**Screen + tile**|`Screen`|**Screen:** `x:0.02, y:0.03, w:0.96, h:0.94`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Tile BR):** `x:0.74, y:0.68, w:0.22, h:0.25`, `r:8%`, `Cover`|**Screen:** `x:0.03, y:0.04, w:0.94, h:0.52`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Tile Bottom):** `x:0.03, y:0.58, w:0.94, h:0.38`, `r:6%`, `Cover`|
|`screen_tile_inv`|**Screen + tile inverted**|`Screen`|**Screen:** `x:0.02, y:0.03, w:0.96, h:0.94`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Tile BL):** `x:0.04, y:0.68, w:0.22, h:0.25`, `r:8%`, `Cover`|**Cam (Tile Top):** `x:0.03, y:0.04, w:0.94, h:0.38`, `r:6%`, `Cover`<br><br>  <br><br>**Screen:** `x:0.03, y:0.44, w:0.94, h:0.52`, `r:3%`, `Contain`|
|`side_by_side`|**Side by side**|`Screen`|**Screen (Left):** `x:0.02, y:0.03, w:0.71, h:0.94`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Right):** `x:0.75, y:0.03, w:0.23, h:0.94`, `r:4%`, `Cover`|**Screen (Top):** `x:0.03, y:0.04, w:0.94, h:0.52`, `r:3%`, `Contain`<br><br>  <br><br>**Cam (Bottom):** `x:0.03, y:0.58, w:0.94, h:0.38`, `r:4%`, `Cover`|
|`side_by_side_inv`|**Side by side inverted**|`Screen`|**Cam (Left):** `x:0.02, y:0.03, w:0.23, h:0.94`, `r:4%`, `Cover`<br><br>  <br><br>**Screen (Right):** `x:0.27, y:0.03, w:0.71, h:0.94`, `r:3%`, `Contain`|**Cam (Top):** `x:0.03, y:0.04, w:0.94, h:0.38`, `r:4%`, `Cover`<br><br>  <br><br>**Screen (Bottom):** `x:0.03, y:0.44, w:0.94, h:0.52`, `r:3%`, `Contain`|
|`screen_focus`|**Screen focus**|`Screen`|**Screen:** `x:0.02, y:0.03, w:0.96, h:0.94`, `r:3%`, `Contain`|**Screen:** `x:0.02, y:0.20, w:0.96, h:0.60`, `r:3%`, `Contain`|

