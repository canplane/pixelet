// pxl-spec/space/coord/mat4.test.rs

use super::*;

fn v(x: f64, y: f64, z: f64) -> V3 {
    [x, y, z]
}

fn assert_almost(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

#[test]
fn inverse_identity() {
    let (mut r, mut t, mut m, mut inv, mut p) = (new(), new(), new(), new(), new());
    rotation_z(&mut r, 0.7);
    translation(&mut t, 5.0, -3.0, 2.0);
    mul(&mut m, &r, &t);
    invert(&mut inv, &m);
    mul(&mut p, &m, &inv);
    let identity = new();
    for k in 0..16 {
        assert_almost(p[k], identity[k], 1e-12);
    }
}

#[test]
fn look_at_axes() {
    let mut view = new();
    look_at(
        &mut view,
        v(0.0, -30.0, 40.0),
        v(0.0, 0.0, 0.0),
        v(0.0, 0.0, 1.0),
    );
    let mut target = [0.0; 3];
    transform_point(&mut target, &view, 0.0, 0.0, 0.0);
    assert_almost(target[0], 0.0, 1e-12);
    assert_almost(target[1], 0.0, 1e-12);
    assert_almost(target[2], -50.0, 1e-12);
    // A point above the target is up on screen.
    let mut above = [0.0; 3];
    transform_point(&mut above, &view, 0.0, 0.0, 1.0);
    assert!(above[1] > 0.0);
}

#[test]
fn frustum_holds_target() {
    let (mut view, mut proj, mut vp) = (new(), new(), new());
    look_at(
        &mut view,
        v(0.0, -30.0, 40.0),
        v(0.0, 0.0, 0.0),
        v(0.0, 0.0, 1.0),
    );
    perspective(&mut proj, core::f64::consts::PI / 3.0, 1.6, 0.1);
    mul(&mut vp, &proj, &view);
    let mut planes = [0.0; 20];
    frustum_planes(&mut planes, &vp);
    let inside = |x: f64, y: f64, z: f64| {
        (0..20)
            .step_by(4)
            .all(|k| planes[k] * x + planes[k + 1] * y + planes[k + 2] * z + planes[k + 3] >= 0.0)
    };
    assert!(inside(0.0, 0.0, 0.0));
    assert!(!inside(0.0, -60.0, 80.0));
}
