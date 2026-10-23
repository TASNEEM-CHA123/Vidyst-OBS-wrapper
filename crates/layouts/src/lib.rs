//! Pure layout geometry for the Vidyst-OBS controller.
//!
//! This crate intentionally has no dependency on OBS or `obws`. Layouts are
//! expressed in normalized coordinates and converted to scene-item transforms
//! by the `obs-driver` crate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const EPSILON: f32 = 0.0001;

/// Supported output canvases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AspectRatio {
    /// 1920x1080 horizontal canvas.
    R16x9,
    /// 1080x1920 vertical canvas.
    R9x16,
    /// 1080x1080 square canvas.
    R1x1,
    /// 1080x1350 portrait canvas.
    R4x5,
}

impl AspectRatio {
    pub const ALL: [Self; 4] = [Self::R16x9, Self::R9x16, Self::R1x1, Self::R4x5];

    /// Canvas dimensions used for transform math.
    pub const fn canvas_size(self) -> (u32, u32) {
        match self {
            Self::R16x9 => (1920, 1080),
            Self::R9x16 => (1080, 1920),
            Self::R1x1 => (1080, 1080),
            Self::R4x5 => (1080, 1350),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::R16x9 => "16:9",
            Self::R9x16 => "9:16",
            Self::R1x1 => "1:1",
            Self::R4x5 => "4:5",
        }
    }
}

/// Layout family displayed in the bottom dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LayoutGroup {
    Solo,
    Screen,
}

impl LayoutGroup {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Solo => "Solo",
            Self::Screen => "Screen",
        }
    }
}

/// A source that can occupy a layout slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SourceId {
    Screen,
    Camera,
    Background,
}

/// How a source fills its slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fit {
    /// Scale until the slot is fully covered; may crop the source.
    Cover,
    /// Scale until the source fits inside the slot; never crops.
    Contain,
}

/// OBS bounds modes used by the layout compiler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundsType {
    /// `OBS_BOUNDS_SCALE_INNER` (value 2).
    ScaleToInnerBounds,
    /// `OBS_BOUNDS_SCALE_OUTER` (value 3).
    ScaleToOuterBounds,
}

impl BoundsType {
    pub const fn obs_value(self) -> i32 {
        match self {
            Self::ScaleToInnerBounds => 2,
            Self::ScaleToOuterBounds => 3,
        }
    }
}

/// Normalized rectangle within a canvas.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Validate normalized bounds and finite values.
    pub fn is_valid(&self) -> bool {
        [self.x, self.y, self.w, self.h]
            .iter()
            .all(|v| v.is_finite())
            && self.x >= 0.0
            && self.y >= 0.0
            && self.w > 0.0
            && self.h > 0.0
            && self.x + self.w <= 1.0 + EPSILON
            && self.y + self.h <= 1.0 + EPSILON
    }

    /// Convert to `(position_x, position_y, width, height)` in canvas pixels.
    pub fn to_pixels(&self, canvas_w: u32, canvas_h: u32) -> (f32, f32, f32, f32) {
        let cw = canvas_w as f32;
        let ch = canvas_h as f32;
        (self.x * cw, self.y * ch, self.w * cw, self.h * ch)
    }
}

/// One source placed in a layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub source: SourceId,
    pub rect: Rect,
    /// 0 = sharp, 8 = rounded tile, 50 = circle.
    pub radius_pct: f32,
    pub fit: Fit,
    pub z: u8,
}

impl Slot {
    /// Pixel width/height of the slot's bounding box.
    pub fn pixel_size(&self, canvas_w: u32, canvas_h: u32) -> (f32, f32) {
        let (_, _, w, h) = self.rect.to_pixels(canvas_w, canvas_h);
        (w, h)
    }

    /// Compile this slot into the transform values OBS expects.
    pub fn obs_transform(&self, canvas_w: u32, canvas_h: u32) -> ObsTransform {
        let (position_x, position_y, bounds_width, bounds_height) =
            self.rect.to_pixels(canvas_w, canvas_h);

        let (bounds_type, crop_to_bounds) = match self.fit {
            Fit::Cover => (BoundsType::ScaleToOuterBounds, true),
            Fit::Contain => (BoundsType::ScaleToInnerBounds, false),
        };

        ObsTransform {
            position_x,
            position_y,
            bounds_width,
            bounds_height,
            // OBS_ALIGN_TOP | OBS_ALIGN_LEFT
            alignment: 5,
            // Bounds alignment center.
            bounds_alignment: 0,
            bounds_type,
            crop_to_bounds,
        }
    }
}

/// Concrete values passed to `SetSceneItemTransform`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObsTransform {
    pub position_x: f32,
    pub position_y: f32,
    pub bounds_width: f32,
    pub bounds_height: f32,
    pub alignment: i32,
    pub bounds_alignment: i32,
    pub bounds_type: BoundsType,
    pub crop_to_bounds: bool,
}

/// A layout contains one slot list per supported aspect ratio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub id: String,
    pub name: String,
    pub group: LayoutGroup,
    pub slots: BTreeMap<AspectRatio, Vec<Slot>>,
}

impl Layout {
    pub fn supports(&self, ratio: AspectRatio) -> bool {
        self.slots.contains_key(&ratio)
    }

    pub fn slots_for(&self, ratio: AspectRatio) -> Option<&[Slot]> {
        self.slots.get(&ratio).map(Vec::as_slice)
    }
}

fn screen(rect: Rect, radius_pct: f32, fit: Fit) -> Slot {
    Slot {
        source: SourceId::Screen,
        rect,
        radius_pct,
        fit,
        z: 0,
    }
}

fn camera(rect: Rect, radius_pct: f32) -> Slot {
    Slot {
        source: SourceId::Camera,
        rect,
        radius_pct,
        fit: Fit::Cover,
        z: 1,
    }
}

fn layout(
    id: &str,
    name: &str,
    group: LayoutGroup,
    horizontal: Vec<Slot>,
    vertical: Vec<Slot>,
) -> Layout {
    let mut slots = BTreeMap::new();
    slots.insert(AspectRatio::R16x9, horizontal);
    slots.insert(AspectRatio::R9x16, vertical);
    Layout {
        id: id.to_owned(),
        name: name.to_owned(),
        group,
        slots,
    }
}

/// The 8 built-in V1 layouts.
///
/// Coordinates mirror the table in `tech_stack.md` §4. Vertical screens use
/// `Fit::Contain` so a 16:9 capture is never horizontally cropped.
pub fn builtin_layouts() -> [Layout; 8] {
    let hd_screen = Rect::new(0.02, 0.03, 0.96, 0.94);

    let just_me = layout(
        "just_me",
        "Just me",
        LayoutGroup::Solo,
        vec![camera(Rect::new(0.02, 0.03, 0.96, 0.94), 4.0)],
        vec![camera(Rect::new(0.03, 0.02, 0.94, 0.96), 4.0)],
    );

    let screen_cam = layout(
        "screen_cam",
        "Screen + cam",
        LayoutGroup::Screen,
        vec![
            screen(hd_screen, 3.0, Fit::Contain),
            camera(Rect::new(0.83, 0.70, 0.140625, 0.25), 50.0),
        ],
        vec![
            screen(Rect::new(0.02, 0.08, 0.96, 0.50), 3.0, Fit::Contain),
            camera(Rect::new(0.29, 0.60, 0.42, 0.23625), 50.0),
        ],
    );

    let screen_cam_inv = layout(
        "screen_cam_inv",
        "Screen + cam inverted",
        LayoutGroup::Screen,
        vec![
            screen(hd_screen, 3.0, Fit::Contain),
            camera(Rect::new(0.03, 0.70, 0.140625, 0.25), 50.0),
        ],
        vec![
            screen(Rect::new(0.02, 0.40, 0.96, 0.50), 3.0, Fit::Contain),
            camera(Rect::new(0.29, 0.06, 0.42, 0.23625), 50.0),
        ],
    );

    let screen_tile = layout(
        "screen_tile",
        "Screen + tile",
        LayoutGroup::Screen,
        vec![
            screen(hd_screen, 3.0, Fit::Contain),
            camera(Rect::new(0.74, 0.68, 0.22, 0.25), 8.0),
        ],
        vec![
            screen(Rect::new(0.03, 0.04, 0.94, 0.52), 3.0, Fit::Contain),
            camera(Rect::new(0.03, 0.58, 0.94, 0.38), 6.0),
        ],
    );

    let screen_tile_inv = layout(
        "screen_tile_inv",
        "Screen + tile inverted",
        LayoutGroup::Screen,
        vec![
            screen(hd_screen, 3.0, Fit::Contain),
            camera(Rect::new(0.04, 0.68, 0.22, 0.25), 8.0),
        ],
        vec![
            screen(Rect::new(0.03, 0.44, 0.94, 0.52), 3.0, Fit::Contain),
            camera(Rect::new(0.03, 0.04, 0.94, 0.38), 6.0),
        ],
    );

    let side_by_side = layout(
        "side_by_side",
        "Side by side",
        LayoutGroup::Screen,
        vec![
            screen(Rect::new(0.02, 0.03, 0.71, 0.94), 3.0, Fit::Contain),
            camera(Rect::new(0.75, 0.03, 0.23, 0.94), 4.0),
        ],
        vec![
            screen(Rect::new(0.03, 0.04, 0.94, 0.52), 3.0, Fit::Contain),
            camera(Rect::new(0.03, 0.58, 0.94, 0.38), 4.0),
        ],
    );

    let side_by_side_inv = layout(
        "side_by_side_inv",
        "Side by side inverted",
        LayoutGroup::Screen,
        vec![
            screen(Rect::new(0.27, 0.03, 0.71, 0.94), 3.0, Fit::Contain),
            camera(Rect::new(0.02, 0.03, 0.23, 0.94), 4.0),
        ],
        vec![
            screen(Rect::new(0.03, 0.44, 0.94, 0.52), 3.0, Fit::Contain),
            camera(Rect::new(0.03, 0.04, 0.94, 0.38), 4.0),
        ],
    );

    let screen_focus = layout(
        "screen_focus",
        "Screen focus",
        LayoutGroup::Screen,
        vec![screen(hd_screen, 3.0, Fit::Contain)],
        vec![screen(Rect::new(0.02, 0.20, 0.96, 0.60), 3.0, Fit::Contain)],
    );

    [
        just_me,
        screen_cam,
        screen_cam_inv,
        screen_tile,
        screen_tile_inv,
        side_by_side,
        side_by_side_inv,
        screen_focus,
    ]
}

/// Look up a built-in layout by its stable ID.
pub fn builtin_layout(id: &str) -> Option<Layout> {
    builtin_layouts().into_iter().find(|layout| layout.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layouts() -> [Layout; 8] {
        builtin_layouts()
    }

    #[test]
    fn has_exactly_eight_unique_layouts() {
        let layouts = layouts();
        let mut ids: Vec<_> = layouts.iter().map(|l| l.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8);
    }

    #[test]
    fn every_layout_has_both_v1_ratios() {
        for layout in layouts() {
            assert!(layout.supports(AspectRatio::R16x9), "{}", layout.id);
            assert!(layout.supports(AspectRatio::R9x16), "{}", layout.id);
        }
    }

    #[test]
    fn rects_are_valid() {
        for layout in layouts() {
            for ratio in [AspectRatio::R16x9, AspectRatio::R9x16] {
                for slot in layout.slots_for(ratio).expect("ratio exists") {
                    assert!(
                        slot.rect.is_valid(),
                        "{} {:?} {:?}",
                        layout.id,
                        ratio,
                        slot.rect
                    );
                }
            }
        }
    }

    #[test]
    fn vertical_screens_always_contain() {
        for layout in layouts() {
            let slots = layout.slots_for(AspectRatio::R9x16).expect("9:16");
            for slot in slots.iter().filter(|s| s.source == SourceId::Screen) {
                assert_eq!(
                    slot.fit,
                    Fit::Contain,
                    "{} must not crop a 16:9 screen on a 9:16 canvas",
                    layout.id
                );
            }
        }
    }

    #[test]
    fn circle_slots_have_equal_pixel_dimensions() {
        for layout in layouts() {
            for ratio in [AspectRatio::R16x9, AspectRatio::R9x16] {
                let (cw, ch) = ratio.canvas_size();
                for slot in layout.slots_for(ratio).expect("ratio exists") {
                    if slot.radius_pct == 50.0 {
                        let (w, h) = slot.pixel_size(cw, ch);
                        assert!(
                            (w - h).abs() < 0.01,
                            "{} {:?}: circle is {}x{}",
                            layout.id,
                            ratio,
                            w,
                            h
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn just_me_has_no_screen_and_screen_focus_has_no_camera() {
        for ratio in [AspectRatio::R16x9, AspectRatio::R9x16] {
            let just_me = builtin_layout("just_me").unwrap();
            assert!(just_me
                .slots_for(ratio)
                .unwrap()
                .iter()
                .all(|s| s.source != SourceId::Screen));

            let screen_focus = builtin_layout("screen_focus").unwrap();
            assert!(screen_focus
                .slots_for(ratio)
                .unwrap()
                .iter()
                .all(|s| s.source != SourceId::Camera));
        }
    }

    #[test]
    fn transform_matches_slot_math() {
        let ratio = AspectRatio::R16x9;
        let (cw, ch) = ratio.canvas_size();
        let screen_cam = builtin_layout("screen_cam").unwrap();
        let cam = screen_cam
            .slots_for(ratio)
            .unwrap()
            .iter()
            .find(|s| s.source == SourceId::Camera)
            .unwrap();

        let transform = cam.obs_transform(cw, ch);
        assert!((transform.position_x - 1593.6).abs() < 0.01);
        assert!((transform.position_y - 756.0).abs() < 0.01);
        assert!((transform.bounds_width - 270.0).abs() < 0.01);
        assert!((transform.bounds_height - 270.0).abs() < 0.01);
        assert_eq!(transform.alignment, 5);
        assert_eq!(transform.bounds_alignment, 0);
        assert_eq!(transform.bounds_type, BoundsType::ScaleToOuterBounds);
        assert!(transform.crop_to_bounds);
    }

    #[test]
    fn contain_maps_to_inner_bounds_without_crop() {
        let ratio = AspectRatio::R9x16;
        let (cw, ch) = ratio.canvas_size();
        let layout = builtin_layout("screen_focus").unwrap();
        let screen = &layout.slots_for(ratio).unwrap()[0];
        let transform = screen.obs_transform(cw, ch);
        assert_eq!(transform.bounds_type, BoundsType::ScaleToInnerBounds);
        assert!(!transform.crop_to_bounds);
    }

    #[test]
    fn side_by_side_split_is_about_73_23() {
        let layout = builtin_layout("side_by_side").unwrap();
        let slots = layout.slots_for(AspectRatio::R16x9).unwrap();
        let screen = slots.iter().find(|s| s.source == SourceId::Screen).unwrap();
        let camera = slots.iter().find(|s| s.source == SourceId::Camera).unwrap();
        assert!((screen.rect.w - 0.71).abs() < EPSILON);
        assert!((camera.rect.w - 0.23).abs() < EPSILON);
    }
}
