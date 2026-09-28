// pxl-kernel/world/observation.test.rs

use super::*;

/// The frustum the view draws, carried into world units, sees what the view
/// looks at: the map center it orbits, from every tilt and turn.
#[test]
fn the_view_s_frustum_sees_the_ground_it_looks_at() {
    let _serial = crate::global::serial();
    let v = view();
    v.origin = ivec::gather(3_000_000, 2_000_000, 0);
    (v.tx, v.ty, v.tz) = (-500.0, -300.0, -10.0);
    (v.width, v.height) = (1600.0, 900.0);
    for (dist, polar, azimuth) in [(50.0, 0.3, 0.0), (5000.0, 1.3, 2.0), (200.0, 0.8, -1.0)] {
        (v.dist, v.polar, v.azimuth) = (dist, polar, azimuth);
        crate::world::frame::view::view_compose();
        let f = frustum_of_view();
        let center = [3_000_000.0 + 500.0, 2_000_000.0 + 300.0, 10.0];
        let v = view();
        let eye = [3_000_000.0 + v.eye_local[0], 2_000_000.0 + v.eye_local[1], v.eye_local[2]];
        assert!(f.sees(center, 1.0), "dist {dist}, polar {polar}, azimuth {azimuth}");
        // And not what lies straight behind the eye.
        let behind = [2.0 * eye[0] - center[0], 2.0 * eye[1] - center[1], center[2]];
        assert!(!f.sees(behind, 1.0), "behind, dist {dist}, polar {polar}, azimuth {azimuth}");
    }
}
