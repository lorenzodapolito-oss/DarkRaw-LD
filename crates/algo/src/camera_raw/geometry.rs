//! Camera Raw's Geometry panel: straighten (rotate), distortion, vertical and horizontal
//! perspective, aspect, scale and offsets. The layer keeps its size; pixels that fall outside
//! the source become transparent (a filter cannot change the canvas, as in Photoshop).
//!
//! Every control is a dimensionless percentage or an angle, and positions are normalised by the
//! half of the long side, so a preview proxy and the full image agree.

use super::CameraRaw;
use crate::photo_util::par_rows;

/// Rotation range of the Rotate slider, in degrees.
pub const ROTATE_RANGE: f32 = 10.0;

impl CameraRaw {
    /// True when every Geometry control is neutral.
    pub(super) fn geometry_neutral(&self) -> bool {
        [self.geo_distortion, self.geo_vertical, self.geo_horizontal, self.geo_rotate, self.geo_aspect, self.geo_scale, self.geo_offset_x, self.geo_offset_y]
            .iter()
            .all(|v| *v == 0.0)
    }
}

/// Maps an output position (normalised, centre origin, y down) to the source position it shows.
fn source_of(p: &CameraRaw, mut u: f32, mut v: f32) -> (f32, f32) {
    // Offsets move the picture within the frame (percent of the long side's half).
    u -= p.geo_offset_x / 100.0;
    v -= p.geo_offset_y / 100.0;
    // Scale: positive zooms in.
    let s = 1.0 + p.geo_scale / 100.0;
    u /= s;
    v /= s;
    // Aspect: positive makes the picture wider (and shorter).
    let f = 2f32.powf(p.geo_aspect / 200.0);
    u /= f;
    v *= f;
    // Rotate by the angle: positive turns the picture clockwise.
    let (sin, cos) = (-p.geo_rotate.to_radians()).sin_cos();
    let (ru, rv) = (u * cos - v * sin, u * sin + v * cos);
    u = ru;
    v = rv;
    // Perspective: vertical narrows the top (positive) or the bottom (negative);
    // horizontal narrows the left or the right.
    let kv = p.geo_vertical / 100.0 * 0.5;
    let kh = p.geo_horizontal / 100.0 * 0.5;
    let dv = (1.0 + kv * v).max(0.05);
    let dh = (1.0 + kh * u).max(0.05);
    u /= dv;
    v /= dh;
    // Distortion: positive is barrel (bulges outward), negative pincushion.
    let d = p.geo_distortion / 100.0 * 0.5;
    let r2 = u * u + v * v;
    let k = (1.0 + d * r2).max(0.05);
    (u / k, v / k)
}

/// Applies the Geometry controls to straight RGBA pixels (`w × h`).
pub(super) fn geometry(px: &mut [[f32; 4]], w: usize, h: usize, p: &CameraRaw) {
    if w < 2 || h < 2 || px.len() < w * h || p.geometry_neutral() {
        return;
    }
    let src: Vec<[f32; 4]> = px.to_vec();
    let half = w.max(h) as f32 / 2.0;
    let (cx, cy) = ((w as f32 - 1.0) / 2.0, (h as f32 - 1.0) / 2.0);
    par_rows(px, w, 1, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let (u, v) = ((x as f32 - cx) / half, (y as f32 - cy) / half);
            let (su, sv) = source_of(p, u, v);
            *out = sample(&src, w, h, cx + su * half, cy + sv * half);
        }
    });
}

/// Bilinear sample with premultiplied colour; outside the image the pixel is transparent, and a
/// half-pixel border fades out so edges are not jagged.
fn sample(src: &[[f32; 4]], w: usize, h: usize, x: f32, y: f32) -> [f32; 4] {
    let (wf, hf) = (w as f32, h as f32);
    if x < -0.5 || y < -0.5 || x > wf - 0.5 || y > hf - 0.5 {
        return [0.0; 4];
    }
    let cover = ((x + 0.5).min(wf - 0.5 - x).clamp(0.0, 1.0)).min((y + 0.5).min(hf - 0.5 - y).clamp(0.0, 1.0));
    let (xc, yc) = (x.clamp(0.0, wf - 1.0), y.clamp(0.0, hf - 1.0));
    let (x0, y0) = (xc.floor() as usize, yc.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (xc - x0 as f32, yc - y0 as f32);
    let mut acc = [0.0f32; 4];
    for (px, wgt) in
        [(src[y0 * w + x0], (1.0 - fx) * (1.0 - fy)), (src[y0 * w + x1], fx * (1.0 - fy)), (src[y1 * w + x0], (1.0 - fx) * fy), (src[y1 * w + x1], fx * fy)]
    {
        let a = px[3];
        acc[0] += px[0] * a * wgt;
        acc[1] += px[1] * a * wgt;
        acc[2] += px[2] * a * wgt;
        acc[3] += a * wgt;
    }
    if acc[3] <= 0.0 {
        return [0.0; 4];
    }
    [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], acc[3] * cover]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: usize, h: usize) -> Vec<[f32; 4]> {
        (0..w * h).map(|i| [(i % w) as f32 / w as f32, (i / w) as f32 / h as f32, 0.5, 1.0]).collect()
    }

    #[test]
    fn neutral_geometry_is_the_identity() {
        let mut px = gradient(32, 24);
        let before = px.clone();
        geometry(&mut px, 32, 24, &CameraRaw::default());
        assert_eq!(px, before);
        assert!(CameraRaw::default().geometry_neutral());
    }

    #[test]
    fn rotating_leaves_transparent_corners_and_keeps_the_centre() {
        let (w, h) = (64, 48);
        let mut px = vec![[0.2, 0.4, 0.6, 1.0]; w * h];
        let p = CameraRaw { geo_rotate: 10.0, ..Default::default() };
        geometry(&mut px, w, h, &p);
        assert!(px[0][3] < 0.01, "a corner is empty after rotating");
        let c = px[(h / 2) * w + w / 2];
        assert!((c[3] - 1.0).abs() < 1e-4 && (c[0] - 0.2).abs() < 1e-3, "the centre keeps its pixel: {c:?}");
    }

    #[test]
    fn opposite_rotations_move_a_marker_to_opposite_sides() {
        let (w, h) = (64, 64);
        let mut base = vec![[0.0, 0.0, 0.0, 1.0]; w * h];
        for y in 10..14 {
            for x in 30..34 {
                base[y * w + x] = [1.0, 1.0, 1.0, 1.0];
            }
        }
        let centroid = |p: &CameraRaw| {
            let mut px = base.clone();
            geometry(&mut px, w, h, p);
            let (mut sx, mut sw) = (0.0, 0.0);
            for (i, q) in px.iter().enumerate() {
                let m = q[0] * q[3];
                sx += (i % w) as f32 * m;
                sw += m;
            }
            sx / sw
        };
        let left = centroid(&CameraRaw { geo_rotate: -10.0, ..Default::default() });
        let right = centroid(&CameraRaw { geo_rotate: 10.0, ..Default::default() });
        assert!(left < 31.0 && right > 33.0, "left {left}, right {right}");
    }

    #[test]
    fn scale_and_offset_move_the_picture() {
        let (w, h) = (40, 40);
        let src = gradient(w, h);
        let mut zoomed = src.clone();
        geometry(&mut zoomed, w, h, &CameraRaw { geo_scale: 50.0, ..Default::default() });
        // Zoomed in: the left edge now shows a column well inside the source.
        assert!(zoomed[20 * w][0] > src[20 * w][0] + 0.1);
        let mut shifted = src.clone();
        geometry(&mut shifted, w, h, &CameraRaw { geo_offset_x: 50.0, ..Default::default() });
        assert!(shifted[20 * w][3] < 0.01, "the left edge is empty after shifting right");
    }

    #[test]
    fn extreme_values_stay_finite() {
        let mut px = gradient(30, 20);
        let p = CameraRaw {
            geo_distortion: 100.0,
            geo_vertical: -100.0,
            geo_horizontal: 100.0,
            geo_rotate: 10.0,
            geo_aspect: -100.0,
            geo_scale: -50.0,
            geo_offset_x: -100.0,
            geo_offset_y: 100.0,
            ..Default::default()
        };
        geometry(&mut px, 30, 20, &p);
        assert!(px.iter().all(|q| q.iter().all(|c| c.is_finite())));
    }
}
