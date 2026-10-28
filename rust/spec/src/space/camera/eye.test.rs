// pxl-spec/space/camera/eye.test.rs

use super::*;
use crate::space::coord::mat4::{self, frustum_planes, look_at, perspective};

/// The ray under an NDC point unprojected through the inverse of
/// `view_proj`, from its eye at the origin: the form the sky's ray took
/// before it read the lens.
fn unprojected(view_proj: &mat4::Mat4, x: f64, y: f64) -> V3 {
    let mut inv = mat4::new();
    mat4::invert(&mut inv, view_proj);
    let clip = [x, y, 0.5, 1.0];
    let row = |r: usize| (0..4).map(|c| inv[c * 4 + r] * clip[c]).sum::<f64>();
    let w = row(3);
    [row(0) / w, row(1) / w, row(2) / w]
}

#[test]
fn a_pixels_ray_off_the_lens_is_the_unprojected_one() {
    let (mut view, mut proj, mut vp) = (mat4::new(), mat4::new(), mat4::new());
    // An eye at the origin looking down past the horizon, as the sky's eye-relative view is.
    look_at(&mut view, [0.0; 3], [3.0, 40.0, -7.0], [0.0, 0.0, 1.0]);
    perspective(&mut proj, core::f64::consts::PI / 3.0, 1.6, 0.1);
    mat4::mul(&mut vp, &proj, &view);
    let mut f = Frustum::default();
    frustum_planes(&mut f.planes, &vp);
    let l = f.lens().expect("a screen");
    for (x, y) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (-0.7, 0.4), (1.0, -1.0)] {
        let ray: V3 = core::array::from_fn(|k| {
            l.forward[k] + x * l.tan[0] * l.right[k] + y * l.tan[1] * l.up[k]
        });
        let a = unprojected(&vp, x, y);
        let cos = v3::dot(&ray, &a) / (v3::len(&ray) * v3::len(&a));
        assert!(1.0 - cos < 1e-12, "at ({x}, {y}) the two part by {}", 1.0 - cos);
    }
}
