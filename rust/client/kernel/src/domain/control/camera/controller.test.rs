// pxl-kernel/domain/control/camera/controller.test.rs

use super::*;
use crate::domain::control::camera::fixture::place;
use crate::domain::view::frame::coord::to_world;
use crate::sys::serial;

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn near(a: V3, b: V3, tol: f64) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() <= tol * a[k].abs().max(1.0))
}

fn third_person() {
    tracked_end();
    controllers().base = CameraMode::orbit;
    settings_store().camera_mode = CameraMode::orbit;
}

/// The orbit's distance third person, none first.
#[test]
fn pivot_along_facing() {
    let _g = serial();
    third_person();
    let mut rng = Rng(0x5eed);
    for n in 0..200 {
        let at = [
            rng.unit() * 2_000.0 - 1_000.0,
            rng.unit() * 2_000.0 - 1_000.0,
            rng.unit() * 40.0,
        ];
        let (dist, polar, azimuth) = (
            crate::domain::control::param::MIN_DIST + rng.unit() * 2_000.0,
            rng.unit() * core::f64::consts::PI,
            rng.unit() * 6.0 - 3.0,
        );
        place(at, dist, polar, azimuth);
        assert!(near(pivot_world().unwrap(), at, 1e-9), "{n}: third person");
        controllers().base = CameraMode::first_person;
        assert!(
            near(pivot_world().unwrap(), eye_world().unwrap(), 1e-12),
            "{n}: first person"
        );
        controllers().base = CameraMode::orbit;
    }
}

/// A change of person moves the camera about the pivot, never the
/// at; under a tracker it waits until the tracker lets go.
#[test]
fn person_change_keeps_pivot() {
    let _g = serial();
    third_person();
    // A canonical point keeps 256ths of a unit: the first change lands within
    // one, and every later change keeps it exactly there.
    let near = |a: V3, b: V3, tol: f64| (0..3).all(|k| (a[k] - b[k]).abs() <= tol);
    let mut rng = Rng(0xc0de);
    for n in 0..100 {
        let at = [
            rng.unit() * 2_000.0 - 1_000.0,
            rng.unit() * 2_000.0 - 1_000.0,
            rng.unit() * 40.0,
        ];
        let (dist, polar, azimuth) = (
            crate::domain::control::param::MIN_DIST + rng.unit() * 2_000.0,
            rng.unit() * core::f64::consts::PI,
            rng.unit() * 6.0 - 3.0,
        );
        place(at, dist, polar, azimuth);
        let mut first: Option<V3> = None;
        for mode in [
            CameraMode::first_person,
            CameraMode::orbit,
            CameraMode::first_person,
            CameraMode::orbit,
        ] {
            assert!(take_mode(mode), "{n}: the change to {mode:?} taken");
            let got = pivot_world().unwrap();
            assert!(
                near(got, at, 2.0 / 256.0),
                "{n}: moved by the change to {mode:?}: {got:?} vs {at:?}"
            );
            let first = *first.get_or_insert(got);
            assert!(
                near(got, first, 1e-6),
                "{n}: drifted by the change to {mode:?}: {got:?} vs {first:?}"
            );
            let to = match mode {
                CameraMode::first_person => {
                    crate::domain::control::camera::first_person::pivot_to()
                }
                CameraMode::orbit => crate::domain::control::camera::orbit::pivot_to(),
            };
            let to = to_world(&to.unwrap());
            assert!(
                near(to, got, 1e-6),
                "{n}: a move under way carried into {mode:?}"
            );
        }
    }
    // Under a tracker the change waits, and the base is left as it was.
    controllers().tracked = Some(Tracked {
        world_turn: mat4::new(),
        world_at: [0.0; 3],
        turn: mat4::new(),
        at: [0.0; 3],
    });
    assert!(
        !take_mode(CameraMode::first_person),
        "taken under a tracker"
    );
    assert_eq!(
        controllers().base,
        CameraMode::orbit,
        "the base changed under a tracker"
    );
    controllers().tracked = None;
    assert!(
        take_mode(CameraMode::first_person),
        "not taken once the tracker let go"
    );
    third_person();
}

#[test]
fn facing_tilt_domain() {
    for n in 0..=360 {
        let polar = n as f64 * core::f64::consts::PI / 360.0;
        for azimuth in [0.0, 1.1, -2.7] {
            let r = facing_rotation(polar, azimuth);
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
            let r = facing_rotation(theta, phi);
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
