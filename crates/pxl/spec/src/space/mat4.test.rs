// pxl-spec/space/mat4.test.rs

use super::*;

fn v(x: f64, y: f64, z: f64) -> V3 {
    [x, y, z]
}

fn assert_almost(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

#[test]
fn mat4_a_matrix_times_its_inverse_is_the_identity() {
    let (mut r, mut t, mut m, mut inv, mut p) = (mat4(), mat4(), mat4(), mat4(), mat4());
    mat4_rotation_z(&mut r, 0.7);
    mat4_translation(&mut t, 5.0, -3.0, 2.0);
    mat4_mul(&mut m, &r, &t);
    mat4_invert(&mut inv, &m);
    mat4_mul(&mut p, &m, &inv);
    let identity = mat4();
    for k in 0..16 {
        assert_almost(p[k], identity[k], 1e-12);
    }
}

#[test]
fn mat4_look_at_puts_the_target_on_the_camera_s_z_axis_up_on_y() {
    let mut view = mat4();
    mat4_look_at(&mut view, v(0.0, -30.0, 40.0), v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
    let mut target = [0.0; 3];
    mat4_transform_point(&mut target, &view, 0.0, 0.0, 0.0);
    assert_almost(target[0], 0.0, 1e-12);
    assert_almost(target[1], 0.0, 1e-12);
    assert_almost(target[2], -50.0, 1e-12);
    // A point above the target is up on screen.
    let mut above = [0.0; 3];
    mat4_transform_point(&mut above, &view, 0.0, 0.0, 1.0);
    assert!(above[1] > 0.0);
}

#[test]
fn mat4_the_frustum_s_planes_hold_the_target_and_reject_what_is_behind_the_eye() {
    let (mut view, mut proj, mut vp) = (mat4(), mat4(), mat4());
    mat4_look_at(&mut view, v(0.0, -30.0, 40.0), v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
    mat4_perspective(&mut proj, core::f64::consts::PI / 3.0, 1.6, 0.1, 1000.0);
    mat4_mul(&mut vp, &proj, &view);
    let mut planes = [0.0; 24];
    mat4_frustum_planes(&mut planes, &vp);
    let inside = |x: f64, y: f64, z: f64| (0..24).step_by(4).all(|k| planes[k] * x + planes[k + 1] * y + planes[k + 2] * z + planes[k + 3] >= 0.0);
    assert!(inside(0.0, 0.0, 0.0));
    assert!(!inside(0.0, -60.0, 80.0));
    assert!(!inside(0.0, 3000.0, 0.0));
}
