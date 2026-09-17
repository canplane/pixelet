// pxl_core/render/view_range.test.rs
//
// The invariant the render range rests on. Ranges are measured from the view
// centre on the ground (one anchor, chunk aligned), while the camera itself can
// stand up to MAX_DIST away from it. That is only safe because the camera always
// looks at the centre: this casts the frustum at the ground over every zoom,
// tilt, aspect ratio and pivot height the camera allows, and checks that ground
// the camera can see — inside the frustum and nearer than fog — lies within the
// distance chunks are mounted to.

use crate::module::camera::config::camera_cfg::projection::FOV;
use crate::module::camera::config::camera_cfg::view::{MAX_DIST, MAX_POLAR, MIN_DIST, MIN_POLAR};
use crate::module::world::environment::config::render_cfg;

/// How far the chunk-aligned anchor can sit from the true view centre.
fn anchor_quantization() -> f64 {
    64f64.hypot(64.0)
}

fn farthest_visible_ground(aspect: f64, pivot_height: f64, steps: i32) -> f64 {
    let vh = FOV * core::f64::consts::PI / 360.0;
    let hh = (vh.tan() * aspect).atan();
    let mut worst: f64 = 0.0;
    for di in 0..=steps {
        let dist = MIN_DIST + (MAX_DIST - MIN_DIST) * di as f64 / steps as f64;
        for pi in 0..=steps {
            let polar = MIN_POLAR + (MAX_POLAR - MIN_POLAR) * pi as f64 / steps as f64;
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
                    if t <= 0.0 || t > render_cfg::FOG_FAR {
                        continue;
                    }
                    let x = t * (d[0] / dl);
                    let y = -horiz + t * (d[1] / dl);
                    worst = worst.max(x.hypot(y));
                }
            }
        }
    }
    worst
}

#[test]
fn view_range_everything_the_camera_can_see_is_inside_the_distance_chunks_are_mounted_to() {
    for aspect in [0.5, 1.0, 16.0 / 9.0, 2.5, 3.0] {
        for pivot_height in [0.0, 50.0, 100.0, 200.0, 400.0] {
            let reach = farthest_visible_ground(aspect, pivot_height, 12) + anchor_quantization();
            assert!(
                reach <= render_cfg::CHUNK_RENDER_DIST,
                "aspect {aspect}, pivot +{pivot_height}u: visible ground reaches {}u, mounted to {}u",
                reach.round(),
                render_cfg::CHUNK_RENDER_DIST
            );
        }
    }
}

#[test]
fn view_range_fog_finishes_before_geometry_does_at_every_pivot_height() {
    assert!(render_cfg::FOG_FAR <= render_cfg::CHUNK_RENDER_DIST);
    assert!(render_cfg::FOG_NEAR < render_cfg::FOG_FAR);
}
