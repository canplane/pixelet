// pxl-kernel/control/camera/controller.test.rs
//
// The controllers make one pose and hand the view between them as it is:
//
// - the free camera taking over from the orbit, and the orbit back from it,
//   leave the eye where it stood and looking the way it looked;
// - a tracker taking the pose leaves it where it was, whatever its first
//   pose, then moves it as the tracker moves; letting go gives back the
//   controller that had it, as it was.

use pxl_spec::space::mat4::mat4;

use super::*;
use crate::control::camera::config::camera_cfg;
use crate::control::camera::free::FreeState;
use crate::control::camera::orbit::OrbitState;
use crate::global::serial;

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn same(a: &ObserverPose, b: &ObserverPose, tol: f64) -> bool {
    (0..3).all(|k| (a.eye[k] - b.eye[k]).abs() <= tol * a.eye[k].abs().max(1.0)) && (0..16).all(|k| (a.rotation[k] - b.rotation[k]).abs() <= 1e-9)
}

/// The orbit handed to the free camera and taken back sees what it saw:
/// the free camera stands at the orbit's eye looking along its look, and the
/// orbit taken back from it stands its pivot on that look — here the sea,
/// or as far as the orbit reached where the look never comes down to it.
#[test]
fn the_orbit_and_the_free_camera_hand_the_view_over_as_it_is() {
    let _g = serial();
    let mut rng = Rng(0x0dd_ba11);
    for n in 0..500 {
        let o = OrbitState {
            pivot: [rng.unit() * 2_000.0 - 1_000.0, rng.unit() * 2_000.0 - 1_000.0, rng.unit() * 40.0],
            dist: camera_cfg::view::MIN_DIST + rng.unit() * 2_000.0,
            // Any tilt: straight down, level, straight up.
            polar: rng.unit() * core::f64::consts::PI,
            azimuth: rng.unit() * 6.0 - 3.0,
            climb: 0.0,
            placed: true,
        };
        let before = orbit_pose(&o);
        crate::control::camera::free::from_orbit(&o);
        let f: &FreeState = free();
        assert!(same(&before, &free_pose(f), 1e-12), "{n}: the free camera moved the view");
        let back = to_orbit(f, o.dist);
        assert!(
            same(&before, &orbit_pose(&back), 1e-9),
            "{n}: the orbit taken back moved the view: {:?} against {:?}",
            back.pivot,
            o.pivot
        );
    }
}

/// A tracked pose as a camera's position and its turn from the camera's
/// axes to the tracker's, as a unit quaternion.
fn about(axis: [f64; 3], angle: f64) -> [f64; 4] {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let s = (angle / 2.0).sin() / n;
    [axis[0] * s, axis[1] * s, axis[2] * s, (angle / 2.0).cos()]
}

/// A tracker takes the pose where it is, whatever it says first; then the
/// camera moves in the world as it moves in the tracker's space, turned as
/// the tracker's space was placed; and letting go gives the view back to
/// the orbit that had it, as it was.
#[test]
fn a_tracker_takes_the_pose_where_it_is_and_gives_it_back() {
    let _g = serial();
    tracked_end();
    *orbit() = OrbitState {
        pivot: [120.0, -40.0, 8.0],
        dist: 300.0,
        polar: 0.9,
        azimuth: 0.4,
        climb: 0.0,
        placed: true,
    };
    view_compose();
    let before = view().pose;
    let first = ([1.2, 0.3, -2.0], about([0.3, 1.0, 0.2], 0.8));
    tracked_pose(first.0, first.1);
    assert_eq!(active(), Active::Tracked);
    assert!(same(&before, &view().pose, 1e-9), "the tracker's first pose moved the view");

    // A step along the tracker's own x, unturned: the eye moves by the
    // world's image of that step, and the camera does not turn.
    tracked_pose([first.0[0] + 5.0, first.0[1], first.0[2]], first.1);
    let after = view().pose;
    let t = controllers().tracked.as_ref().unwrap();
    let step = apply3(&t.world_turn, [5.0, 0.0, 0.0]);
    assert!(
        (0..3).all(|k| (after.eye[k] - before.eye[k] - step[k]).abs() < 1e-9),
        "{:?} against {:?} + {step:?}",
        after.eye,
        before.eye
    );
    assert!((0..16).all(|k| (after.rotation[k] - before.rotation[k]).abs() < 1e-9), "a step turned the camera");

    // A turn in place: the eye stays, and the camera turns.
    tracked_pose([first.0[0] + 5.0, first.0[1], first.0[2]], about([0.0, 1.0, 0.0], 0.5));
    assert!((0..3).all(|k| (view().pose.eye[k] - after.eye[k]).abs() < 1e-9), "a turn moved the eye");
    assert!(view().pose.rotation != mat4(), "the camera did not turn");

    tracked_end();
    assert_eq!(active(), Active::Orbit);
    assert!(same(&before, &view().pose, 1e-9), "the orbit came back moved");
}

/// A navigation command far off carries the free camera before the turn
/// observes: after the effects' snap alone, what it looks at is where the
/// command put the map, and it looks the way it did.
#[test]
fn a_navigation_command_carries_the_free_camera_before_the_turn_observes() {
    let _g = serial();
    tracked_end();
    *orbit() = OrbitState {
        pivot: [0.0, 0.0, 0.0],
        dist: 400.0,
        polar: 0.7,
        azimuth: 1.1,
        climb: 0.0,
        placed: true,
    };
    view_compose();
    crate::control::camera::free::from_orbit(orbit());
    controllers().base = CameraMode::free;
    view_compose();
    let turn = view().pose.rotation;
    let mut target = pxl_spec::space::pvf3::pvf3::Pvf3::default();
    pxl_spec::space::pvf3::projection::project(&mut target, 250_000.0, -90_000.0, 0.0);
    map_store().navigate_to(target);
    snap_step();
    let f = free();
    let look = crate::control::camera::free::look_point(f.eye, crate::control::camera::orbit::look_dir(f.pitch, f.yaw), 400.0);
    let mut seen = pxl_spec::space::pvf3::pvf3::Pvf3::default();
    local_world_to_canonical(&mut seen, &look, view().origin);
    let apart = |a: &pxl_spec::space::pvf3::pvf3::Pvf3| {
        let mut p = [0.0; 3];
        pxl_spec::space::pvf3::projection::unproject(&mut p, a.i, a.f);
        p
    };
    let (s, t) = (apart(&seen), apart(&target));
    assert!((0..3).all(|k| (s[k] - t[k]).abs() < 1e-6), "the free camera looks at {s:?}, not {t:?}");
    assert!((0..16).all(|k| (view().pose.rotation[k] - turn[k]).abs() < 1e-12), "the command turned the camera");
    controllers().base = CameraMode::orbit;
}
