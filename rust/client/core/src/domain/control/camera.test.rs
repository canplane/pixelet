// pxl-core/domain/control/camera.test.rs

use super::*;

#[test]
fn facing_tilt_domain() {
    for n in 0..=360 {
        let polar = n as f64 * core::f64::consts::PI / 360.0;
        for azimuth in [0.0, 1.1, -2.7] {
            let r = facing_rot(polar, azimuth);
            let row = |i: usize| [r[i], r[4 + i], r[8 + i]];
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            for i in 0..3 {
                for j in 0..3 {
                    let want = if i == j { 1.0 } else { 0.0 };
                    assert!(
                        (dot(row(i), row(j)) - want).abs() < 1e-12,
                        "tilt {polar}, heading {azimuth}: not a rotation"
                    );
                }
            }
            let back = row(2);
            let forward = facing_dir(polar, azimuth);
            assert!(
                (0..3).all(|k| (forward[k] + back[k]).abs() < 1e-12),
                "tilt {polar}, heading {azimuth}: looks along {:?}, not {forward:?}",
                back.map(|v| -v)
            );
        }
    }
}

/// Spherical coordinates' own degeneracy: the turn is still a rotation there.
#[test]
fn pole_headings_agree() {
    let pi = core::f64::consts::PI;
    for theta in [0.0, pi] {
        let first = facing_dir(theta, 0.0);
        for phi in [0.7, -2.1, 3.0] {
            let d = facing_dir(theta, phi);
            assert!(
                (0..3).all(|k| (d[k] - first[k]).abs() < 1e-12),
                "θ {theta}: φ {phi} looks along {d:?}, not {first:?}"
            );
        }
    }
    for theta in [0.0, 1e-12, 1e-6, pi - 1e-6, pi - 1e-12, pi] {
        for phi in [0.0, 1.3, -2.9] {
            let r = facing_rot(theta, phi);
            assert!(r.iter().all(|v| v.is_finite()), "θ {theta}, φ {phi}: {r:?}");
            let row = |i: usize| [r[i], r[4 + i], r[8 + i]];
            for i in 0..3 {
                let n = row(i).iter().map(|v| v * v).sum::<f64>();
                assert!(
                    (n - 1.0).abs() < 1e-12,
                    "θ {theta}, φ {phi}: an axis of length {n}"
                );
            }
        }
    }
}
