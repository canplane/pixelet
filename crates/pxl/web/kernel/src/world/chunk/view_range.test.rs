// pxl-kernel/world/chunk/view_range.test.rs
//
// The invariant the render range rests on. Ranges are measured from the view
// centre on the ground (one anchor, chunk aligned), while the camera itself can
// stand up to MAX_DIST away from it. That is only safe because the camera always
// looks at the centre: this casts the frustum at the ground over every zoom,
// tilt, aspect ratio and pivot height the camera allows, and checks that ground
// the camera can see — inside the frustum and nearer than fog — lies within
// what the level rings draw.

use crate::control::camera::config::camera_cfg::projection::FOV;
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MAX_POLAR, MIN_DIST, MIN_POLAR};
use crate::layer::environment::config::render_cfg;
use crate::layer::environment::fog::{band_at, reach_at};
use crate::layer::terrain::level::{drawn_from_eye, drawn_reach, window_for, WINDOW_LEVELS};
use xpute_core::math::scalar::round;

/// How far the chunk-aligned anchor can sit from the true view centre.
fn anchor_quantization() -> f64 {
    64f64.hypot(64.0)
}

/// The farthest ground one view shows — the camera `dist` from a pivot
/// `pivot_height` above the ground, at `polar` from straight down — inside
/// `far`, where the band has taken it, measured from the view centre.
fn farthest_visible_ground(aspect: f64, pivot_height: f64, dist: f64, polar: f64, steps: i32, far: f64) -> f64 {
    let vh = FOV * core::f64::consts::PI / 360.0;
    let hh = (vh.tan() * aspect).atan();
    let mut worst: f64 = 0.0;
    let horiz = dist * polar.sin();
    let height = pivot_height + dist * polar.cos();
    let len = horiz.hypot(dist * polar.cos());
    let len = if len == 0.0 { 1.0 } else { len };
    let f = [0.0, horiz / len, -dist * polar.cos() / len];
    let r = [1.0, 0.0, 0.0];
    let u = [f[1] * r[2] - f[2] * r[1], f[2] * r[0] - f[0] * r[2], f[0] * r[1] - f[1] * r[0]];
    for yi in -steps..=steps {
        let ty = (hh * yi as f64 / steps as f64).tan();
        for qi in -steps..=steps {
            let tp = (vh * qi as f64 / steps as f64).tan();
            let d = [f[0] + ty * r[0] + tp * u[0], f[1] + ty * r[1] + tp * u[1], f[2] + ty * r[2] + tp * u[2]];
            let dl = d[0].hypot(d[1]).hypot(d[2]);
            let dz = d[2] / dl;
            if dz >= 0.0 {
                continue;
            }
            let t = height / -dz;
            if t <= 0.0 || t > far {
                continue;
            }
            let x = t * (d[0] / dl);
            let y = -horiz + t * (d[1] / dl);
            worst = worst.max(x.hypot(y));
        }
    }
    worst
}

#[test]
fn view_range_everything_the_camera_can_see_is_inside_what_the_rings_draw() {
    // Each view against its own window: the ground it shows must lie inside
    // what its window's top level's ring draws.
    const STEPS: i32 = 12;
    const DIST_STEPS: i32 = 34;
    for aspect in [0.5, 1.0, 16.0 / 9.0, 2.5, 3.0] {
        for pivot_height in [0.0, 50.0, 100.0, 200.0, 400.0] {
            for di in 0..=DIST_STEPS {
                // Geometric: the range spans seventeen octaves, and every
                // octave is a view of its own.
                let dist = MIN_DIST * (MAX_DIST / MIN_DIST).powf(di as f64 / DIST_STEPS as f64);
                for pi in 0..=STEPS {
                    let polar = MIN_POLAR + (MAX_POLAR - MIN_POLAR) * pi as f64 / STEPS as f64;
                    let window = window_for(dist, polar, pivot_height, aspect);
                    let drawn = drawn_reach(window.end - 1);
                    // What the eye is shown: the band ends the ground at the
                    // nearer of the reach and what the rings draw, so a view
                    // whose air and planet leave more than the window holds
                    // is faded out rather than cut (environment/fog.rs).
                    let far = band_at(pivot_height + dist * polar.cos(), drawn_from_eye(dist, polar, window.end - 1)).1;
                    let reach = farthest_visible_ground(aspect, pivot_height, dist, polar, STEPS, far) + anchor_quantization();
                    assert!(
                        reach <= drawn,
                        "aspect {aspect}, pivot +{pivot_height}u, dist {dist}, polar {polar:.2}: visible ground reaches {}u, window {window:?} draws to {}u",
                        round(reach),
                        drawn
                    );
                }
            }
        }
    }
}

#[test]
fn the_band_fades_out_inside_the_reach_at_every_height_a_view_has() {
    const { assert!(render_cfg::FOG_NEAR < render_cfg::FOG_FAR) };
    for height in [0.0, 1.0, 50.0, 400.0, 4_096.0, 262_144.0] {
        let drawn = drawn_reach(WINDOW_LEVELS - 1);
        let (near, far) = band_at(height, drawn);
        assert!(near < far, "at {height}u the band has to fade before it ends");
        assert!(far <= reach_at(height).min(drawn), "at {height}u the ground ends at the nearer of the reach and what is drawn");
    }
}
