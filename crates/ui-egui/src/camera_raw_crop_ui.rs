//! The Camera Raw dialog's crop frame and guide grid, drawn over the preview and set from the
//! Geometry panel, as in Photoshop's Crop tool.
//!
//! * The **frame** has the ratio presets Photoshop offers (Original, 1 : 1, 4 : 5, 5 : 7, 3 : 4,
//!   2 : 3, 16 : 9, 16 : 10, or Free), a swap for portrait and landscape, and handles to move and
//!   resize it. Outside it the picture is dimmed and inside it the chosen guide shows (Rule of
//!   Thirds, Grid, …). The frame crops the picture when a raw file is opened (the document gets the
//!   frame's size); a filter keeps its layer's size, so there the frame is off.
//! * The **grid** covers the whole picture while a Geometry slider is dragged (Rotate, Scale, …),
//!   so the horizon and verticals can be judged against it, or always when asked for.
//!
//! The frame is kept as a rectangle in the picture's unit square (`0..=1` on both axes), so it
//! follows the picture through zoom and the preview proxy.

use egui::{Color32, CursorIcon, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::crop_overlay::{CropOverlay, guides};
use crate::widgets;

/// Ratio presets: (key, label). `""` shows no frame and `free` a frame of any shape. A ratio key
/// is `long:short`; the frame takes the picture's orientation, and Swap turns it.
pub const RATIOS: &[(&str, &str)] = &[
    ("", "No Frame"),
    ("free", "Free"),
    ("original", "Original Ratio"),
    ("1:1", "1 : 1 (Square)"),
    ("4:5", "4 : 5 (8 : 10)"),
    ("5:7", "5 : 7"),
    ("3:4", "3 : 4 (6 : 8)"),
    ("2:3", "2 : 3 (4 : 6)"),
    ("16:9", "16 : 9"),
    ("16:10", "16 : 10"),
];

/// The smallest side the frame can be shrunk to, as a fraction of the picture.
const MIN_SIDE: f32 = 0.03;
/// Screen points around a corner (half the side of its square) and an edge (half its thickness)
/// that grab the frame.
const CORNER_HIT: f32 = 12.0;
const EDGE_HIT: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Axis {
    X,
    Y,
}

/// What a drag on the frame moves: the picture under it (the frame stays put on screen, as in
/// Photoshop's Crop tool), one corner (`-1.0` is the left/top side, `1.0` the right/bottom one) or
/// one edge.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    Image,
    Corner(f32, f32),
    Edge(Axis, f32),
}

#[derive(Clone, Debug)]
pub struct CropFrame {
    /// A key of [`RATIOS`]; empty for no frame.
    pub ratio: String,
    /// Portrait for a landscape picture and the other way round.
    pub flipped: bool,
    /// The frame as `[left, top, right, bottom]` in the picture's unit square.
    pub rect: [f32; 4],
    /// How far the picture is drawn from where the preview puts it, as a fraction of its size:
    /// moving the picture under the frame shifts the frame over the picture and this the other
    /// way, so the frame stays where it is on screen.
    pub offset: [f32; 2],
    /// The guide drawn inside the frame.
    pub overlay: CropOverlay,
    /// Show the grid over the picture even when no Geometry slider is being dragged.
    pub grid_always: bool,
    grab: Option<Grab>,
}

impl Default for CropFrame {
    fn default() -> Self {
        Self { ratio: String::new(), flipped: false, rect: [0.0, 0.0, 1.0, 1.0], offset: [0.0, 0.0], overlay: CropOverlay::Thirds, grid_always: false, grab: None }
    }
}

impl CropFrame {
    /// A frame is shown (and crops on opening a raw file).
    pub fn active(&self) -> bool {
        !self.ratio.is_empty()
    }

    /// Width / height, in pixels, of the chosen ratio for a `w` × `h` picture; None for a free
    /// frame or none.
    pub fn target(&self, w: f32, h: f32) -> Option<f32> {
        if !(w > 0.0 && h > 0.0) {
            return None;
        }
        let r = match self.ratio.as_str() {
            "" | "free" => return None,
            "original" => w / h,
            key => {
                let (a, b) = crate::chrome_ui::crop_ratio(key, 1.0, 1.0)?;
                let (long, short) = (a.max(b) as f32, a.min(b) as f32);
                if !(short > 0.0 && long.is_finite()) {
                    return None;
                }
                if w >= h { long / short } else { short / long }
            }
        };
        Some(if self.flipped { 1.0 / r } else { r })
    }

    /// The ratio in the unit square: frame width over frame height, both as fractions.
    fn unit_ratio(&self, w: f32, h: f32) -> Option<f32> {
        self.target(w, h).map(|r| r * h / w)
    }

    /// The largest frame of the chosen ratio, centred where the frame is now.
    pub fn fit(&mut self, w: f32, h: f32) {
        let Some(ru) = self.unit_ratio(w, h).filter(|r| r.is_finite() && *r > 0.0) else { return };
        let (fw, fh) = if ru >= 1.0 { (1.0, 1.0 / ru) } else { (ru, 1.0) };
        let [x0, y0, x1, y1] = self.rect;
        let cx = ((x0 + x1) / 2.0).clamp(fw / 2.0, 1.0 - fw / 2.0);
        let cy = ((y0 + y1) / 2.0).clamp(fh / 2.0, 1.0 - fh / 2.0);
        self.rect = [cx - fw / 2.0, cy - fh / 2.0, cx + fw / 2.0, cy + fh / 2.0];
    }

    /// Choose a ratio preset. A fixed ratio fits the largest frame; Free keeps the frame.
    pub fn set_ratio(&mut self, key: &str, w: f32, h: f32) {
        self.ratio = key.to_string();
        self.flipped = false;
        self.offset = [0.0, 0.0];
        match key {
            "" => self.rect = [0.0, 0.0, 1.0, 1.0],
            "free" => {}
            _ => self.fit(w, h),
        }
    }

    /// Turn a fixed-ratio frame between portrait and landscape.
    pub fn flip(&mut self, w: f32, h: f32) {
        if self.target(w, h).is_some() {
            self.flipped = !self.flipped;
            self.offset = [0.0, 0.0];
            self.fit(w, h);
        }
    }

    /// Back to the largest frame of the current ratio.
    pub fn reset(&mut self, w: f32, h: f32) {
        self.offset = [0.0, 0.0];
        self.rect = [0.0, 0.0, 1.0, 1.0];
        self.fit(w, h);
    }

    /// The frame in pixels of a `w` × `h` picture as `[x, y, width, height]`; None with no frame
    /// or when it covers the whole picture.
    pub fn pixels(&self, w: usize, h: usize) -> Option<[i32; 4]> {
        if !self.active() || w == 0 || h == 0 {
            return None;
        }
        let [x0, y0, x1, y1] = self.rect;
        let (wi, hi) = (w as i32, h as i32);
        let left = ((x0 * w as f32).round() as i32).clamp(0, wi - 1);
        let top = ((y0 * h as f32).round() as i32).clamp(0, hi - 1);
        let right = ((x1 * w as f32).round() as i32).clamp(left + 1, wi);
        let bottom = ((y1 * h as f32).round() as i32).clamp(top + 1, hi);
        let (cw, ch) = (right - left, bottom - top);
        (cw < wi || ch < hi).then_some([left, top, cw, ch])
    }

    /// Where the picture is drawn, given where the preview puts it (`img`).
    pub fn shifted(&self, img: Rect) -> Rect {
        img.translate(vec2(self.offset[0] * img.width(), self.offset[1] * img.height()))
    }

    /// The frame on screen, over the picture drawn in `img`.
    fn screen(&self, img: Rect) -> Rect {
        let [x0, y0, x1, y1] = self.rect;
        Rect::from_min_max(pos2(img.left() + x0 * img.width(), img.top() + y0 * img.height()), pos2(img.left() + x1 * img.width(), img.top() + y1 * img.height()))
    }

    /// Apply a drag: `p` is the pointer and `delta` its movement, in the picture's unit square.
    fn apply(&mut self, grab: Grab, p: [f32; 2], delta: [f32; 2], w: f32, h: f32) {
        let [x0, y0, x1, y1] = self.rect;
        let ru = self.unit_ratio(w, h).filter(|r| r.is_finite() && *r > 0.0);
        let p = [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)];
        self.rect = match grab {
            Grab::Image => {
                // The picture moves with the pointer, so the frame moves the other way over it
                // (until it meets the picture's edge) and the picture is drawn shifted to match.
                let (fw, fh) = (x1 - x0, y1 - y0);
                let nx = (x0 - delta[0]).clamp(0.0, (1.0 - fw).max(0.0));
                let ny = (y0 - delta[1]).clamp(0.0, (1.0 - fh).max(0.0));
                self.offset[0] -= nx - x0;
                self.offset[1] -= ny - y0;
                [nx, ny, nx + fw, ny + fh]
            }
            Grab::Corner(sx, sy) => {
                // The opposite corner stays where it is.
                let (ax, ay) = (if sx < 0.0 { x1 } else { x0 }, if sy < 0.0 { y1 } else { y0 });
                let (max_w, max_h) = (if sx < 0.0 { ax } else { 1.0 - ax }, if sy < 0.0 { ay } else { 1.0 - ay });
                let (mut fw, mut fh) = ((p[0] - ax).abs().max(MIN_SIDE), (p[1] - ay).abs().max(MIN_SIDE));
                if let Some(ru) = ru {
                    fw = fw.max(fh * ru);
                    fh = fw / ru;
                    let k = (max_w / fw).min(max_h / fh).min(1.0);
                    fw *= k;
                    fh *= k;
                } else {
                    fw = fw.min(max_w);
                    fh = fh.min(max_h);
                }
                [if sx < 0.0 { ax - fw } else { ax }, if sy < 0.0 { ay - fh } else { ay }, if sx < 0.0 { ax } else { ax + fw }, if sy < 0.0 { ay } else { ay + fh }]
            }
            Grab::Edge(Axis::X, side) => {
                let anchor = if side < 0.0 { x1 } else { x0 };
                let max_w = if side < 0.0 { x1 } else { 1.0 - x0 };
                let mut fw = (p[0] - anchor).abs().max(MIN_SIDE).min(max_w);
                let (mut ny0, mut ny1) = (y0, y1);
                if let Some(ru) = ru {
                    let mut fh = fw / ru;
                    if fh > 1.0 {
                        fh = 1.0;
                        fw = fh * ru;
                    }
                    let cy = ((y0 + y1) / 2.0).clamp(fh / 2.0, 1.0 - fh / 2.0);
                    (ny0, ny1) = (cy - fh / 2.0, cy + fh / 2.0);
                }
                [if side < 0.0 { anchor - fw } else { anchor }, ny0, if side < 0.0 { anchor } else { anchor + fw }, ny1]
            }
            Grab::Edge(Axis::Y, side) => {
                let anchor = if side < 0.0 { y1 } else { y0 };
                let max_h = if side < 0.0 { y1 } else { 1.0 - y0 };
                let mut fh = (p[1] - anchor).abs().max(MIN_SIDE).min(max_h);
                let (mut nx0, mut nx1) = (x0, x1);
                if let Some(ru) = ru {
                    let mut fw = fh * ru;
                    if fw > 1.0 {
                        fw = 1.0;
                        fh = fw / ru;
                    }
                    let cx = ((x0 + x1) / 2.0).clamp(fw / 2.0, 1.0 - fw / 2.0);
                    (nx0, nx1) = (cx - fw / 2.0, cx + fw / 2.0);
                }
                [nx0, if side < 0.0 { anchor - fh } else { anchor }, nx1, if side < 0.0 { anchor } else { anchor + fh }]
            }
        };
    }
}

/// Paint the grid (when `grid`) and the frame over the picture shown in `img`.
pub fn paint(painter: &Painter, frame: &CropFrame, img: Rect, grid: bool) {
    let line = Stroke::new(1.0, Color32::from_white_alpha(110));
    let draw = |at: Rect, kind: CropOverlay| {
        for polyline in guides(kind, 0, at.width(), at.height()) {
            let points: Vec<Pos2> = polyline.iter().map(|[x, y]| pos2(at.left() + x * at.width(), at.top() + y * at.height())).collect();
            painter.add(egui::Shape::line(points, line));
        }
    };
    if grid {
        draw(img, CropOverlay::Grid);
    }
    if !frame.active() {
        return;
    }
    let f = frame.screen(img);
    let dim = Color32::from_black_alpha(150);
    for r in [
        Rect::from_min_max(img.min, pos2(img.right(), f.top())),
        Rect::from_min_max(pos2(img.left(), f.bottom()), img.max),
        Rect::from_min_max(pos2(img.left(), f.top()), pos2(f.left(), f.bottom())),
        Rect::from_min_max(pos2(f.right(), f.top()), pos2(img.right(), f.bottom())),
    ] {
        painter.rect_filled(r, 0.0, dim);
    }
    if !grid {
        draw(f, frame.overlay);
    }
    painter.rect_stroke(f, 0.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Outside);
    let handle = |c: Pos2| {
        let r = Rect::from_center_size(c, vec2(7.0, 7.0));
        painter.rect_filled(r, 1.0, Color32::WHITE);
        painter.rect_stroke(r, 1.0, Stroke::new(1.0, Color32::from_black_alpha(200)), StrokeKind::Outside);
    };
    for c in [f.left_top(), f.right_top(), f.left_bottom(), f.right_bottom(), f.center_top(), f.center_bottom(), f.left_center(), f.right_center()] {
        handle(c);
    }
}

/// Handle the pointer on the frame: its corners and edges resize it, and dragging inside moves the
/// picture under it, to choose what is cropped. `img` is where the picture is drawn
/// ([`CropFrame::shifted`]). True when the frame changed. `size` is the picture in pixels.
pub fn interact(ui: &mut Ui, frame: &mut CropFrame, img: Rect, size: (f32, f32)) -> bool {
    if !frame.active() || img.width() < 1.0 || img.height() < 1.0 {
        return false;
    }
    let f = frame.screen(img);
    let id = ui.id().with("cr-crop-frame");
    let unit = |p: Pos2| [(p.x - img.left()) / img.width(), (p.y - img.top()) / img.height()];
    // Later zones sit on top: corners over edges over the inside.
    let (e, c) = (EDGE_HIT, CORNER_HIT);
    let inner = f.shrink2(vec2(e.min(f.width() / 3.0), e.min(f.height() / 3.0)));
    let mut zones: Vec<(u8, Rect, Grab, CursorIcon)> = Vec::new();
    // The inside only grabs when the picture has room to move under the frame; else a drag still
    // zooms and pans.
    if f.width() < img.width() * 0.98 || f.height() < img.height() * 0.98 {
        zones.push((0, inner, Grab::Image, CursorIcon::Grab));
    }
    let (hmid, vmid) = (f.shrink2(vec2(0.0, e)), f.shrink2(vec2(e, 0.0)));
    zones.push((1, Rect::from_min_max(pos2(f.left() - e, hmid.top()), pos2(f.left() + e, hmid.bottom())), Grab::Edge(Axis::X, -1.0), CursorIcon::ResizeHorizontal));
    zones.push((2, Rect::from_min_max(pos2(f.right() - e, hmid.top()), pos2(f.right() + e, hmid.bottom())), Grab::Edge(Axis::X, 1.0), CursorIcon::ResizeHorizontal));
    zones.push((3, Rect::from_min_max(pos2(vmid.left(), f.top() - e), pos2(vmid.right(), f.top() + e)), Grab::Edge(Axis::Y, -1.0), CursorIcon::ResizeVertical));
    zones.push((4, Rect::from_min_max(pos2(vmid.left(), f.bottom() - e), pos2(vmid.right(), f.bottom() + e)), Grab::Edge(Axis::Y, 1.0), CursorIcon::ResizeVertical));
    for (slot, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].into_iter().enumerate() {
        let at = pos2(if sx < 0.0 { f.left() } else { f.right() }, if sy < 0.0 { f.top() } else { f.bottom() });
        let cursor = if sx * sy > 0.0 { CursorIcon::ResizeNwSe } else { CursorIcon::ResizeNeSw };
        zones.push((5 + slot as u8, Rect::from_center_size(at, vec2(2.0 * c, 2.0 * c)), Grab::Corner(sx, sy), cursor));
    }
    let mut changed = false;
    for (slot, rect, grab, cursor) in zones {
        let r = ui.interact(rect, id.with(slot), Sense::drag());
        if r.hovered() || r.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if r.drag_started() {
            frame.grab = Some(grab);
        }
        if r.dragged()
            && frame.grab == Some(grab)
            && let Some(p) = r.interact_pointer_pos()
        {
            let d = r.drag_delta();
            frame.apply(grab, unit(p), [d.x / img.width(), d.y / img.height()], size.0, size.1);
            changed = true;
        }
        if r.drag_stopped() && frame.grab == Some(grab) {
            frame.grab = None;
        }
    }
    changed
}

/// The Geometry panel's frame controls. `enabled` is false where a frame can't crop (a filter
/// keeps its layer's size); `size` is the picture in pixels.
pub fn controls(ui: &mut Ui, frame: &mut CropFrame, size: (f32, f32), enabled: bool) {
    widgets::section_label(ui, "Crop Frame");
    ui.add_enabled_ui(enabled, |ui| {
        let mut key = frame.ratio.clone();
        let ratios: Vec<(String, &str)> = RATIOS.iter().map(|(k, l)| (k.to_string(), *l)).collect();
        if widgets::dropdown(ui, "cr-crop-ratio", &mut key, &ratios, 220.0) {
            frame.set_ratio(&key, size.0, size.1);
        }
        let fixed = frame.target(size.0, size.1).is_some();
        ui.horizontal(|ui| {
            if ui.add_enabled(fixed, egui::Button::new(tl!("Swap"))).on_hover_text(tl!("Turn the frame between portrait and landscape")).clicked() {
                frame.flip(size.0, size.1);
            }
            if ui.add_enabled(frame.active(), egui::Button::new(tl!("Reset Frame"))).clicked() {
                frame.reset(size.0, size.1);
            }
        });
        let overlays: Vec<(CropOverlay, &str)> = CropOverlay::ALL.iter().map(|k| (*k, k.label())).collect();
        widgets::dropdown(ui, "cr-crop-overlay", &mut frame.overlay, &overlays, 220.0);
    });
    if !enabled {
        let t = crate::theme::Tokens::get(ui.ctx());
        ui.label(egui::RichText::new(tl!("The frame crops when a raw file is opened; a filter keeps the layer's size.")).color(t.text_faint));
    }
    widgets::checkbox(ui, &mut frame.grid_always, tl!("Always show grid"));
}

/// The frame for the control channel's `crop` entry: `{ratio, flipped, rect, overlay, gridAlways,
/// pixels, available}`. `size` is the picture in pixels; `available` is false where a frame can't
/// crop (a filter keeps its layer's size).
pub fn describe(frame: &CropFrame, size: (usize, usize), available: bool) -> Value {
    json!({"ratio": frame.ratio, "flipped": frame.flipped, "rect": frame.rect, "overlay": frame.overlay.id(), "gridAlways": frame.grid_always,
        "pixels": frame.pixels(size.0, size.1), "available": available})
}

/// Validate a control-channel `crop` update (`ratio`, `flipped`, `rect`, `overlay`, `gridAlways`)
/// against the current frame, without changing it. A filter takes only the grid settings.
pub fn prepare(current: &CropFrame, value: &Value, size: (f32, f32), available: bool) -> Result<CropFrame, String> {
    const KEYS: [&str; 5] = ["ratio", "flipped", "rect", "overlay", "gridAlways"];
    let obj = value.as_object().ok_or("Camera Raw crop must be an object")?;
    if let Some(k) = obj.keys().find(|k| !KEYS.contains(&k.as_str())) {
        return Err(format!("unknown Camera Raw crop setting {k}"));
    }
    if !available && ["ratio", "flipped", "rect"].iter().any(|k| obj.contains_key(*k)) {
        return Err("a filter keeps the layer's size: the crop frame is only for opening a raw file".into());
    }
    let mut next = current.clone();
    if let Some(v) = obj.get("ratio") {
        let key = v.as_str().filter(|k| RATIOS.iter().any(|(r, _)| r == k)).ok_or("unknown Camera Raw crop ratio")?;
        next.set_ratio(key, size.0, size.1);
    }
    if let Some(v) = obj.get("flipped") {
        let flipped = v.as_bool().ok_or("Camera Raw crop `flipped` must be true or false")?;
        if flipped != next.flipped {
            next.flip(size.0, size.1);
        }
    }
    if let Some(v) = obj.get("rect") {
        let rect: Vec<f32> = v.as_array().map(|a| a.iter().filter_map(|n| n.as_f64().map(|n| n as f32)).collect()).unwrap_or_default();
        let ok = rect.len() == 4
            && v.as_array().is_some_and(|a| a.len() == 4)
            && rect.iter().all(|n| n.is_finite() && (0.0..=1.0).contains(n))
            && rect[2] - rect[0] >= MIN_SIDE
            && rect[3] - rect[1] >= MIN_SIDE;
        if !ok {
            return Err("Camera Raw crop rect must be [left, top, right, bottom] within 0..1".into());
        }
        if !next.active() {
            next.ratio = "free".into();
        }
        next.rect = [rect[0], rect[1], rect[2], rect[3]];
        next.offset = [0.0, 0.0];
    }
    if let Some(v) = obj.get("overlay") {
        next.overlay = v.as_str().and_then(CropOverlay::from_id).ok_or("unknown Camera Raw crop overlay")?;
    }
    if let Some(v) = obj.get("gridAlways") {
        next.grid_always = v.as_bool().ok_or("Camera Raw crop `gridAlways` must be true or false")?;
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 6000.0;
    const H: f32 = 4000.0;

    fn framed(key: &str) -> CropFrame {
        let mut f = CropFrame::default();
        f.set_ratio(key, W, H);
        f
    }

    fn px_ratio(f: &CropFrame) -> f32 {
        let [x0, y0, x1, y1] = f.rect;
        (x1 - x0) * W / ((y1 - y0) * H)
    }

    #[test]
    fn no_frame_is_the_whole_picture_and_crops_nothing() {
        let f = CropFrame::default();
        assert!(!f.active());
        assert_eq!(f.pixels(6000, 4000), None);
        assert_eq!(framed("original").pixels(6000, 4000), None, "the original ratio is the whole picture");
    }

    #[test]
    fn presets_fit_the_largest_frame_with_the_picture_orientation() {
        for (key, want) in [("1:1", 1.0), ("4:5", 1.25), ("3:4", 4.0 / 3.0), ("2:3", 1.5), ("16:9", 16.0 / 9.0), ("5:7", 1.4), ("16:10", 1.6)] {
            let f = framed(key);
            assert!((px_ratio(&f) - want).abs() < 1e-3, "{key}: {} vs {want}", px_ratio(&f));
            let [x0, y0, x1, y1] = f.rect;
            assert!(x0 >= -1e-6 && y0 >= -1e-6 && x1 <= 1.0 + 1e-6 && y1 <= 1.0 + 1e-6, "{key} stays inside the picture: {:?}", f.rect);
            assert!(((x1 - x0) - 1.0).abs() < 1e-5 || ((y1 - y0) - 1.0).abs() < 1e-5, "{key} is the largest frame");
        }
        // A portrait picture gets portrait frames.
        let mut p = CropFrame::default();
        p.set_ratio("3:4", 4000.0, 6000.0);
        let [x0, y0, x1, y1] = p.rect;
        assert!(((x1 - x0) * 4000.0 / ((y1 - y0) * 6000.0) - 0.75).abs() < 1e-3);
    }

    #[test]
    fn swap_turns_the_frame_and_free_has_no_ratio() {
        let mut f = framed("16:9");
        f.flip(W, H);
        assert!((px_ratio(&f) - 9.0 / 16.0).abs() < 1e-3);
        f.flip(W, H);
        assert!((px_ratio(&f) - 16.0 / 9.0).abs() < 1e-3);
        let mut free = framed("free");
        assert_eq!(free.target(W, H), None);
        let before = free.rect;
        free.flip(W, H);
        assert_eq!(free.rect, before);
    }

    #[test]
    fn crop_pixels_match_the_frame() {
        let f = framed("1:1");
        let [x, y, w, h] = f.pixels(6000, 4000).expect("a square frame crops a 3:2 picture");
        assert_eq!((w, h), (4000, 4000));
        assert_eq!(y, 0);
        assert_eq!(x, 1000, "centred");
    }

    #[test]
    fn corner_and_edge_drags_keep_the_ratio_and_stay_inside() {
        let mut f = framed("16:9");
        // Pull the bottom-right corner far past the picture: still inside, still 16:9.
        f.apply(Grab::Corner(1.0, 1.0), [3.0, 3.0], [0.0, 0.0], W, H);
        assert!((px_ratio(&f) - 16.0 / 9.0).abs() < 1e-3, "{}", px_ratio(&f));
        let [x0, y0, x1, y1] = f.rect;
        assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= 1.0 + 1e-6 && y1 <= 1.0 + 1e-6, "{:?}", f.rect);
        // Shrink from the top-left towards the middle.
        f.apply(Grab::Corner(-1.0, -1.0), [0.5, 0.5], [0.0, 0.0], W, H);
        assert!((px_ratio(&f) - 16.0 / 9.0).abs() < 1e-3);
        assert!(f.rect[2] - f.rect[0] < 0.7, "{:?}", f.rect);
        // Drag the left edge out (the frame already meets the right edge): the width grows and the
        // height follows around the centre.
        let before = f.rect;
        f.apply(Grab::Edge(Axis::X, -1.0), [before[0] - 0.1, 0.5], [0.0, 0.0], W, H);
        assert!(f.rect[0] < before[0], "{:?} vs {before:?}", f.rect);
        assert!((px_ratio(&f) - 16.0 / 9.0).abs() < 1e-3);
    }

    #[test]
    fn free_frames_resize_on_each_axis_and_move_within_the_picture() {
        let mut f = framed("free");
        f.apply(Grab::Edge(Axis::X, 1.0), [0.5, 0.5], [0.0, 0.0], W, H);
        assert_eq!(f.rect, [0.0, 0.0, 0.5, 1.0]);
        f.apply(Grab::Edge(Axis::Y, -1.0), [0.5, 0.25], [0.0, 0.0], W, H);
        assert_eq!(f.rect, [0.0, 0.25, 0.5, 1.0]);
        // Dragging the picture right and up slides the frame left and down over it, to the edge.
        f.apply(Grab::Image, [0.0, 0.0], [-0.9, 0.9], W, H);
        assert_eq!(f.rect, [0.5, 0.0, 1.0, 0.75], "moves until it meets the picture's edge");
        assert_eq!(f.offset, [-0.5, 0.25], "the picture is drawn shifted by what the frame moved");
        f.apply(Grab::Edge(Axis::X, 1.0), [0.5, 0.0], [0.0, 0.0], W, H);
        assert!(f.rect[2] - f.rect[0] >= MIN_SIDE - 1e-6, "never collapses: {:?}", f.rect);
    }

    #[test]
    fn dragging_on_the_preview_resizes_the_frame() {
        let ctx = egui::Context::default();
        crate::PhotocraftApp::setup_context(&ctx, crate::theme::ThemeKind::ALL[0]);
        let img = Rect::from_min_size(pos2(100.0, 100.0), vec2(600.0, 400.0));
        let mut frame = framed("free");
        frame.rect = [0.2, 0.2, 0.8, 0.8];
        let run = |events: Vec<egui::Event>, frame: &mut CropFrame| {
            let input = egui::RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), ..Default::default() };
            ctx.run_ui(input, |ui| {
                interact(ui, frame, img, (W, H));
            })
            .textures_delta
            .clear();
        };
        let corner = pos2(100.0 + 0.8 * 600.0, 100.0 + 0.8 * 400.0);
        let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        run(vec![egui::Event::PointerMoved(corner)], &mut frame);
        run(vec![button(corner, true)], &mut frame);
        let to = pos2(100.0 + 0.9 * 600.0, 100.0 + 0.9 * 400.0);
        run(vec![egui::Event::PointerMoved(to)], &mut frame);
        run(vec![egui::Event::PointerMoved(to)], &mut frame);
        run(vec![button(to, false)], &mut frame);
        assert!(frame.rect[2] > 0.85 && frame.rect[3] > 0.85, "the corner followed the pointer: {:?}", frame.rect);
        assert_eq!((frame.rect[0], frame.rect[1]), (0.2, 0.2), "the opposite corner stayed");
    }
}
