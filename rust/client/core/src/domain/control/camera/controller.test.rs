// pxl-core/domain/control/camera/controller.test.rs

use super::*;
use crate::domain::control::camera::first_person;
use crate::domain::control::camera::fixture::place;
use crate::domain::control::camera::orbit;
use crate::domain::control::param;
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

/// A pose drawn from `rng` about the world's middle, placed: its pivot.
fn placed(rng: &mut Rng) -> V3 {
    let at = [
        rng.unit() * 2_000.0 - 1_000.0,
        rng.unit() * 2_000.0 - 1_000.0,
        rng.unit() * 40.0,
    ];
    let dist = param::MIN_DIST + rng.unit() * 2_000.0;
    let polar = rng.unit() * core::f64::consts::PI;
    place(at, dist, polar, rng.unit() * 6.0 - 3.0);
    at
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
        let at = placed(&mut rng);
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
        let at = placed(&mut rng);
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
                CameraMode::first_person => first_person::pivot_to(),
                CameraMode::orbit => orbit::pivot_to(),
            };
            let to = to_world(&to.unwrap());
            assert!(
                near(to, got, 1e-6),
                "{n}: a move under way carried into {mode:?}"
            );
        }
    }
    // Under a tracker the change waits, and the base is left as it was.
    controllers().tracker = Some(TrackerSpace {
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
    controllers().tracker = None;
    assert!(
        take_mode(CameraMode::first_person),
        "not taken once the tracker let go"
    );
    third_person();
}
