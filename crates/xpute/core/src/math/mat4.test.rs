// xpute-core/math/mat4.test.rs

use super::*;

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
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
    let mut target = Vec3::default();
    mat4_transform_point(&mut target, &view, 0.0, 0.0, 0.0);
    assert_almost(target.x, 0.0, 1e-12);
    assert_almost(target.y, 0.0, 1e-12);
    assert_almost(target.z, -50.0, 1e-12);
    // A point above the target is up on screen.
    let mut above = Vec3::default();
    mat4_transform_point(&mut above, &view, 0.0, 0.0, 1.0);
    assert!(above.y > 0.0);
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

/// What the TypeScript computed, recorded: the generator that wrote it
/// is gone and the vector stands as it is (xpute-core/sys/vector.rs).
const GOLDEN: &str = include_str!("../../golden/math/mat4.tsv");

#[test]
fn mat4_computes_what_the_record_holds() {
    let v = crate::golden::Golden::load(GOLDEN);
    // Exact where the arithmetic alone decides; a few ulps where a value
    // passed through a transcendental function: `libm`'s msun against V8's fdlibm.
    let exact = |name: &str, got: &[f64]| {
        for (j, g) in got.iter().enumerate() {
            assert_eq!(g.to_bits(), v.f64(&format!("{name}.{j}")).to_bits(), "{name}.{j}: {g}");
        }
    };
    let near = |name: &str, got: &[f64]| {
        for (j, g) in got.iter().enumerate() {
            let w = v.f64(&format!("{name}.{j}"));
            let d = if g.to_bits() == w.to_bits() {
                0
            } else if g.is_sign_negative() != w.is_sign_negative() {
                u64::MAX
            } else {
                g.abs().to_bits().abs_diff(w.abs().to_bits())
            };
            assert!(d <= 8 || (g - w).abs() < 1e-12, "{name}.{j}: {g} vs {w}");
        }
    };
    let (mut r, mut t, mut m, mut persp, mut look, mut up, mut vp, mut inv, mut singular) = (mat4(), mat4(), mat4(), mat4(), mat4(), mat4(), mat4(), mat4(), mat4());
    mat4_rotation_z(&mut r, 0.7);
    mat4_translation(&mut t, 5.0, -3.0, 2.25);
    mat4_mul(&mut m, &r, &t);
    mat4_perspective(&mut persp, core::f64::consts::PI / 3.0, 1.6, 0.1, 1000.0);
    mat4_look_at(&mut look, v3(0.5, -30.0, 40.0), v3(1.0, 2.0, 0.0), v3(0.0, 0.0, 1.0));
    mat4_look_at(&mut up, v3(0.0, 0.0, 10.0), v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 1.0));
    mat4_mul(&mut vp, &persp, &look);
    mat4_invert(&mut inv, &vp);
    mat4_invert(&mut singular, &[0.0; 16]);
    near("rotation_z", &r);
    exact("translation", &t);
    near("mul", &m);
    near("perspective", &persp);
    exact("look_at", &look);
    exact("look_up", &up);
    near("vp", &vp);
    near("invert", &inv);
    exact("singular", &singular);
    let mut p = Vec3::default();
    mat4_transform_point(&mut p, &vp, 3.0, -4.0, 5.0);
    near("point", &[p.x, p.y, p.z]);
    let mut d = Vec3::default();
    mat4_transform_direction(&mut d, &look, 1.0, 2.0, 3.0);
    exact("direction", &[d.x, d.y, d.z]);
    let mut nan_w = mat4();
    mat4_translation(&mut nan_w, 1.0, 2.0, 3.0);
    nan_w[15] = f64::NAN;
    let mut q = Vec3::default();
    mat4_transform_point(&mut q, &nan_w, 3.0, -4.0, 5.0);
    exact("nan_w_point", &[q.x, q.y, q.z]);
    let (w, want) = (mat4_clip_w(&vp, 3.0, -4.0, 5.0), v.f64("clip_w"));
    assert!((w - want).abs() < 1e-12, "clip_w: {w} vs {want}");
    let mut planes = [0.0; 24];
    mat4_frustum_planes(&mut planes, &vp);
    near("planes", &planes);
}

fn v3(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
}
