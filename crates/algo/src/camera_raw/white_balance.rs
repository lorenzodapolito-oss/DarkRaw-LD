//! Camera Raw white-balance picking: from a sample that should be neutral grey (the White
//! Balance tool's click) or from the whole picture's average (Auto), find the Temperature and
//! Tint that make it neutral.
//!
//! The sliders are [`white_balance_gains`](super::white_balance_gains): red `1 + 0.35 t`,
//! green `1 − 0.25 n`, blue `1 − 0.35 t` with `t`, `n` in −1..=1 (slider ÷ 100), applied in
//! linear light. Neutral means `r·g_r = g·g_g = b·g_b`, which solves in closed form:
//! `t = (b − r) / (0.35 (r + b))`, then `n = 4 (1 − (1 + 0.35 t) r / g)`.

use super::srgb_decode;

/// Darkest sample (linear, brightest channel) a white-balance pick trusts.
const MIN_LIGHT: f32 = 0.01;

/// Temperature and Tint (slider units) that turn the display-encoded RGB `sample` neutral, or
/// `None` for a sample that carries no usable colour (not finite, almost black).
pub fn neutral_white_balance(sample: [f32; 3]) -> Option<(f32, f32)> {
    if !sample.iter().all(|v| v.is_finite()) {
        return None;
    }
    let [r, g, b] = sample.map(|v| srgb_decode(v.clamp(0.0, 1.0)));
    if r.max(g).max(b) < MIN_LIGHT {
        return None;
    }
    // A channel at zero has no ratio to correct; keep the maths finite.
    let [r, g, b] = [r, g, b].map(|v| v.max(1e-3));
    let t = ((b - r) / (0.35 * (r + b))).clamp(-1.0, 1.0);
    let n = (4.0 * (1.0 - (1.0 + 0.35 * t) * r / g)).clamp(-1.0, 1.0);
    let round = |v: f32| (v * 1000.0).round() / 10.0;
    Some((round(t), round(n)))
}

/// [`neutral_white_balance`] of the average colour (in linear light) of the visible pixels:
/// the "grey world" estimate behind Auto white balance.
pub fn average_white_balance(px: &[[f32; 4]]) -> Option<(f32, f32)> {
    let mut sum = [0.0f64; 3];
    let mut n = 0u64;
    for q in px {
        if q[3].is_nan() || q[3] <= 0.0 || !q.iter().all(|v| v.is_finite()) {
            continue;
        }
        for c in 0..3 {
            sum[c] += f64::from(srgb_decode(q[c].clamp(0.0, 1.0)));
        }
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let mean = sum.map(|s| super::linear_to_srgb((s / n as f64) as f32));
    neutral_white_balance(mean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera_raw::{CameraRaw, develop};

    fn develop_one(c: [f32; 3], t: f32, n: f32) -> [f32; 3] {
        let mut px = [[c[0], c[1], c[2], 1.0]];
        develop(&mut px, 1, 1, &CameraRaw { temperature: t, tint: n, ..Default::default() }, false);
        [px[0][0], px[0][1], px[0][2]]
    }

    #[test]
    fn a_picked_cast_becomes_neutral() {
        for (t, n) in [(30.0, 0.0), (-40.0, 10.0), (0.0, -25.0), (55.0, 20.0), (-20.0, -15.0)] {
            // A neutral grey seen through a colour cast…
            let cast = develop_one([0.5, 0.5, 0.5], t, n);
            // …picked, then corrected: the three channels agree again.
            let (pt, pn) = neutral_white_balance(cast).unwrap();
            let fixed = develop_one(cast, pt, pn);
            let spread = fixed.iter().cloned().fold(f32::MIN, f32::max) - fixed.iter().cloned().fold(f32::MAX, f32::min);
            assert!(spread < 0.02, "cast ({t}, {n}) → pick ({pt}, {pn}) → {fixed:?}");
        }
    }

    #[test]
    fn a_neutral_sample_changes_nothing() {
        assert_eq!(neutral_white_balance([0.4, 0.4, 0.4]), Some((0.0, 0.0)));
    }

    #[test]
    fn a_warm_sample_is_cooled_and_a_green_one_gets_magenta() {
        let (t, _) = neutral_white_balance([0.7, 0.5, 0.3]).unwrap();
        assert!(t < 0.0, "{t}");
        let (_, n) = neutral_white_balance([0.4, 0.6, 0.4]).unwrap();
        assert!(n > 0.0, "{n}");
    }

    #[test]
    fn unusable_samples_are_refused_and_results_stay_on_the_sliders() {
        assert_eq!(neutral_white_balance([0.0, 0.0, 0.0]), None);
        assert_eq!(neutral_white_balance([f32::NAN, 0.5, 0.5]), None);
        assert_eq!(neutral_white_balance([f32::INFINITY, 0.5, 0.5]), None);
        for s in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [5.0, -3.0, 0.2], [1.0, 1.0, 1.0]] {
            if let Some((t, n)) = neutral_white_balance(s) {
                assert!((-100.0..=100.0).contains(&t) && (-100.0..=100.0).contains(&n), "{s:?} → ({t}, {n})");
            }
        }
    }

    #[test]
    fn the_average_of_a_tinted_picture_is_corrected() {
        let px: Vec<[f32; 4]> = (0..64)
            .map(|i| {
                let v = 0.2 + 0.6 * (i as f32 / 63.0);
                [v, v * 0.85, v * 0.6, 1.0]
            })
            .collect();
        let (t, _) = average_white_balance(&px).unwrap();
        assert!(t < -5.0, "a warm picture is cooled, got {t}");
        assert_eq!(average_white_balance(&[]), None);
        assert_eq!(average_white_balance(&[[0.5, 0.5, 0.5, 0.0]]), None);
        assert_eq!(average_white_balance(&[[f32::NAN, 0.5, 0.5, 1.0]]), None);
    }
}
