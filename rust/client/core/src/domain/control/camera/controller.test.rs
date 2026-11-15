// pxl-core/domain/control/camera/controller.test.rs

use super::*;
use crate::domain::control::camera::fixture::place;
use crate::domain::control::param;
use crate::domain::setting::{OrbitObstacleMode, settings_mut};
use crate::domain::view::frame::coord::to_world;

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
    controller().base = CameraMode::Orbit;
    settings_mut().camera_mode = CameraMode::Orbit;
}

/// The pivot lies the orbit's distance ahead of the eye in third person,
/// and at the eye in first person.
#[test]
fn pivot_along_facing() {
    third_person();
    let mut rng = Rng(0x5eed);
    for n in 0..200 {
        let at = placed(&mut rng);
        assert!(near(pivot_world().unwrap(), at, 1e-9), "{n}: third person");
        controller().base = CameraMode::FirstPerson;
        assert!(
            near(pivot_world().unwrap(), eye_world().unwrap(), 1e-12),
            "{n}: first person"
        );
        controller().base = CameraMode::Orbit;
    }
}

/// A trackpad's pinch takes the camera as near as two touches spread by the
/// same factor do, and first person ignores it.
#[test]
fn a_trackpad_pinch_zooms_as_two_touches() {
    third_person();
    let was = settings().orbit_obstacle;
    settings_mut().orbit_obstacle = OrbitObstacleMode::Through;
    let at = [0.0, 0.0, 10.0];
    place(at, 400.0, 1.0, 0.0);
    pinch_by(2.0);
    let by_trackpad = camera_state().intent.dist;
    place(at, 400.0, 1.0, 0.0);
    pinch_begin(100.0);
    pinch(200.0);
    assert_eq!(by_trackpad, camera_state().intent.dist);
    assert!((by_trackpad - 200.0).abs() < 1e-9, "{by_trackpad}");
    controller().base = CameraMode::FirstPerson;
    pinch_by(2.0);
    assert_eq!(camera_state().intent.dist, 200.0, "first person zoomed");
    controller().base = CameraMode::Orbit;
    settings_mut().orbit_obstacle = was;
}

/// A change of person moves the camera about the pivot and never moves the
/// pivot. Under a tracker, the change waits until the tracker lets go.
#[test]
fn person_change_keeps_pivot() {
    third_person();
    // A canonical point holds 256ths of a unit. The first change lands within
    // two of them, and every later change keeps the pivot exactly there.
    let near = |a: V3, b: V3, tol: f64| (0..3).all(|k| (a[k] - b[k]).abs() <= tol);
    let mut rng = Rng(0xc0de);
    for n in 0..100 {
        let at = placed(&mut rng);
        let mut first: Option<V3> = None;
        for mode in [
            CameraMode::FirstPerson,
            CameraMode::Orbit,
            CameraMode::FirstPerson,
            CameraMode::Orbit,
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
            let to = to_world(&pivot_to().unwrap());
            assert!(
                near(to, got, 1e-6),
                "{n}: a move under way carried into {mode:?}"
            );
        }
    }
    // Under a tracker the change waits, and the base is left as it was.
    controller().tracker = Some(TrackerSpace {
        world_rot: mat4::new(),
        world_at: [0.0; 3],
        rot: mat4::new(),
        at: [0.0; 3],
    });
    assert!(!take_mode(CameraMode::FirstPerson), "taken under a tracker");
    assert_eq!(
        controller().base,
        CameraMode::Orbit,
        "the base changed under a tracker"
    );
    controller().tracker = None;
    assert!(
        take_mode(CameraMode::FirstPerson),
        "not taken once the tracker let go"
    );
    third_person();
}
