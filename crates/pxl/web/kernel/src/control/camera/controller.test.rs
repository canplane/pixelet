// pxl-kernel/control/camera/controller.test.rs
//
// The camera is an eye and a look, and the map center is derived from it:
//
// - the map center is the eye pushed the radius along the look, the eye
//   itself first person;
// - a turn keeps the point it turns about: the orbit's the map center, the
//   free camera's the eye;
// - a navigation command puts the camera so the map center is where it says,
//   and drops what the hand had still to move it by;
// - the free camera walks over the ground as far whichever way it goes and
//   wherever it stands, within the lattice's rows, and no gesture moves its
//   eye up or down;
// - third person to first stands the person at the map center, no earlier
//   height put back; first to third lays the map center on the ground and
//   steps the eye back by the radius;
// - a tracker taking the pose leaves it where it was, whatever its first
//   pose, then moves it as the tracker moves; letting go gives back the
//   controller that had it, as it was, and while it holds the pose the map
//   center is its eye;
// - the look's coordinates hold over their whole domain: every tilt a
//   rotation, and at either pole every heading the one direction.

use pxl_spec::space::mat4::mat4;

use super::*;
use crate::control::camera::config::camera_cfg;
use crate::control::camera::free::pan;
use crate::control::camera::orbit::orbit_eye_offset;
use crate::global::serial;
use crate::world::fixture::place;
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pvf3::{ivec, projection};

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

/// The map center in units.
fn units(c: &Pvf3) -> V3 {
    let mut p = [0.0; 3];
    projection::unproject(&mut p, c.i, c.f);
    p
}

/// The eye in units.
fn eye_units() -> V3 {
    let (e, o) = (camera().eye.unwrap(), view().origin);
    [e[0] + ivec::to_real(ivec::X(o)), e[1] + ivec::to_real(ivec::Y(o)), e[2] + ivec::to_real(ivec::Z(o))]
}

/// The map center in units, as the camera's frame holds it: not rounded to
/// a canonical point's 256th, which is wider than the tolerance far out.
fn center_units() -> V3 {
    let (c, o) = (center_local().unwrap(), view().origin);
    [c[0] + ivec::to_real(ivec::X(o)), c[1] + ivec::to_real(ivec::Y(o)), c[2] + ivec::to_real(ivec::Z(o))]
}

fn near(a: V3, b: V3, tol: f64) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() <= tol * a[k].abs().max(1.0))
}

/// The controllers back to third person, as every test leaves them.
fn third_person() {
    tracked_end();
    controllers().base = CameraMode::orbit;
    settings_store().camera_mode = CameraMode::orbit;
}

/// The map center is the eye pushed the radius along the look: the orbit's
/// distance third person, none first.
#[test]
fn the_map_center_is_the_eye_pushed_the_radius_along_the_look() {
    let _g = serial();
    third_person();
    let mut rng = Rng(0x5eed);
    for n in 0..200 {
        let center = [rng.unit() * 2_000.0 - 1_000.0, rng.unit() * 2_000.0 - 1_000.0, rng.unit() * 40.0];
        let (dist, polar, azimuth) = (camera_cfg::view::MIN_DIST + rng.unit() * 2_000.0, rng.unit() * core::f64::consts::PI, rng.unit() * 6.0 - 3.0);
        place(center, dist, polar, azimuth);
        assert!(near(center_local().unwrap(), center, 1e-9), "{n}: third person");
        controllers().base = CameraMode::free;
        assert!(near(center_local().unwrap(), eye_local().unwrap(), 1e-12), "{n}: first person");
        controllers().base = CameraMode::orbit;
    }
}

/// A turn keeps the point it turns about: the orbit's the map center as the
/// look eases to a new heading and tilt, the free camera's its eye.
#[test]
fn a_turn_keeps_the_point_it_turns_about() {
    let _g = serial();
    third_person();
    place([120.0, -40.0, 8.0], 300.0, 0.9, 0.4);
    let before = center_units();
    camera_store().intent.azimuth = 2.0;
    camera_store().intent.polar = 1.2;
    for _ in 0..30 {
        crate::control::camera::orbit::turn(1.0 / 60.0);
    }
    assert!(near(center_units(), before, 1e-9), "the orbit's turn moved the map center");

    controllers().base = CameraMode::free;
    let eye = eye_units();
    camera_store().intent.azimuth = -1.0;
    for _ in 0..30 {
        crate::control::camera::free::turn(1.0 / 60.0);
    }
    assert!(near(eye_units(), eye, 1e-9), "the free camera's turn moved the eye");
    third_person();
}

/// A navigation command puts the camera so the map center is where it
/// says, whichever controller has it, and the look is kept.
#[test]
fn a_navigation_command_puts_the_map_center_where_it_says() {
    let _g = serial();
    third_person();
    let mut target = Pvf3::default();
    projection::project(&mut target, 250_000.0, -90_000.0, 30.0);
    // The second origin stands past the Z lane's half, where the eye stands
    // above it: a point read into the frame across the lane's wrap would land
    // a lane's length away.
    for (mode, origin_z) in [(CameraMode::orbit, 0), (CameraMode::free, 0), (CameraMode::orbit, 32_768), (CameraMode::free, 32_768)] {
        view().origin = ivec::gather(0, 0, origin_z);
        place([0.0, 0.0, 0.0], 400.0, 0.7, 1.1);
        controllers().base = mode;
        let look = (camera().pitch, camera().yaw);
        put_center(target);
        assert!(near(center_units(), units(&target), 1e-9), "{mode:?}: the map center is not where the command put it");
        assert_eq!((camera().pitch, camera().yaw), look, "{mode:?}: the command turned the camera");
    }
    third_person();
}

/// Third person to first puts the eye at the map center, and no height is
/// put back; first to third lays the map center on the ground — here, with
/// no ground resident, where it stands — and steps the eye back by the
/// radius, looking the way it looked.
#[test]
fn a_change_of_person_moves_the_eye_to_the_center_and_back_by_the_radius() {
    let _g = serial();
    third_person();
    place([120.0, -40.0, 8.0], 300.0, 0.9, 0.4);
    let center = units(&map_center().unwrap());

    settings_store().camera_mode = CameraMode::free;
    switch();
    assert_eq!(active(), Active::Free);
    assert!(near(eye_units(), center, 1e-9), "first person stands elsewhere than the map center");

    settings_store().camera_mode = CameraMode::orbit;
    switch();
    assert_eq!(active(), Active::Orbit);
    assert!(near(center_units(), center, 1e-9), "third person looks elsewhere than where first person stood");
    let e = orbit_eye_offset(orbit().dist, camera().pitch, camera().yaw);
    assert!(near(eye_units(), std::array::from_fn(|k| center[k] + e[k]), 1e-9), "the eye did not step back by the radius");
    third_person();
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
    third_person();
    place([120.0, -40.0, 8.0], 300.0, 0.9, 0.4);
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
    assert!(near(center_local().unwrap(), view().pose.eye, 1e-12), "the map center is not the tracked eye");

    tracked_end();
    assert_eq!(active(), Active::Orbit);
    assert!(same(&before, &view().pose, 1e-9), "the orbit came back moved");
}

/// First person, the controls where the look is, so nothing eases.
fn first_person() {
    tracked_end();
    settings_store().camera_mode = CameraMode::free;
    switch();
    let cam = camera();
    camera_store().commit_intent(Intent {
        dist: orbit().dist,
        azimuth: cam.yaw,
        polar: cam.pitch,
    });
}

/// The free camera's turns, until what the hand asked is walked.
fn walked() {
    while camera_store().travel != [0.0; 2] {
        crate::control::camera::free::turn(1.0 / 60.0);
    }
}

/// The free camera walks within the lattice's rows: put north past them,
/// it stands on the last.
#[test]
fn the_free_camera_stands_within_the_lattice() {
    let _g = serial();
    first_person();
    set_eye_local(&[0.0, 1.0e9, 100.0]);
    crate::control::camera::free::hold();
    assert_eq!(eye_units()[1], REACH.max[1] - 1.0, "carried north");
    third_person();
}

/// The free camera walks as far whichever way it goes and wherever it
/// stands: a hundred pixels of drag across or along carry the eye one
/// distance over the ground, low or high, looking down or up, once the walk
/// has settled. Its height is the ground's, and none is known here, so no
/// walk and no zoom moves it.
#[test]
fn the_free_camera_walks_one_distance_every_way_and_no_higher() {
    let _g = serial();
    first_person();
    let moves: [fn(); 3] = [|| pan(100.0, 0.0), || pan(0.0, 100.0), || wheel(100.0)];
    let mut first = None;
    for (z, pitch) in [(400.0, 0.8), (40_000.0, 2.6), (2_000.0, 0.0)] {
        for (n, m) in moves.into_iter().enumerate() {
            set_eye_local(&[0.0, 0.0, z]);
            let cam = camera();
            (cam.yaw, cam.pitch) = (0.3, pitch);
            camera_store().commit_intent(Intent {
                dist: orbit().dist,
                azimuth: 0.3,
                polar: pitch,
            });
            view_compose();
            let a = eye_units();
            m();
            walked();
            let b = eye_units();
            assert_eq!(b[2], a[2], "at z {z}, pitch {pitch}: the eye went up or down");
            let by = (b[0] - a[0]).hypot(b[1] - a[1]);
            if n == 2 {
                assert_eq!(by, 0.0, "at z {z}, pitch {pitch}: the wheel walked the first person");
                continue;
            }
            let first = *first.get_or_insert(by);
            assert!(by > 0.0 && (by - first).abs() <= 1e-9 * first, "at z {z}, pitch {pitch}: {by}, not {first}");
        }
    }
    third_person();
}

/// A place said outright drops what the hand had still to move the camera
/// by: nothing walks it off the place afterwards.
#[test]
fn a_place_put_drops_the_hand_s_move() {
    let _g = serial();
    first_person();
    set_eye_local(&[0.0, 0.0, 100.0]);
    view_compose();
    pan(500.0, 0.0);
    let target = map_center().unwrap();
    put_center(target);
    let at = eye_units();
    assert!(!crate::control::camera::free::turn(1.0 / 60.0), "still moving after a put");
    assert!(near(eye_units(), at, 1e-12), "walked off the place it was put at");
    third_person();
}

/// A pinch is the fingers' spread against where it began, and keeps no
/// debt: brought back untouched by a limit it asks the distance it began
/// at; driven past one, the share refused is gone, and the first closing of
/// the fingers zooms back out from where the limit held it. No ground is
/// resident here, so the gate allows what is asked and the limits are the
/// distance's own.
#[test]
fn a_pinch_keeps_no_debt() {
    let _g = serial();
    third_person();
    place([0.0, 0.0, 10.0], 300.0, 0.9, 0.4);
    let from = camera_store().intent.dist;
    pinch_begin(100.0);
    for span in [80.0, 120.0, 100.0] {
        pinch(span);
    }
    assert_eq!(camera_store().intent.dist, from, "brought back, the pinch did not ask where it began");
    pinch(1.0e-3);
    assert_eq!(camera_store().intent.dist, camera_cfg::view::MAX_DIST, "past the farthest");
    pinch(2.0e-3);
    assert_eq!(camera_store().intent.dist, camera_cfg::view::MAX_DIST / 2.0, "the closing went to the refused share");
    third_person();
}

/// A look tilts over the whole of its domain: at every tilt, straight down
/// to straight up, the camera's turn is a rotation, it looks the way its
/// heading and tilt say, and nothing in it comes apart.
#[test]
fn a_look_tilts_over_its_whole_domain() {
    for n in 0..=360 {
        let polar = n as f64 * core::f64::consts::PI / 360.0;
        for azimuth in [0.0, 1.1, -2.7] {
            let r = look_rotation(polar, azimuth);
            let row = |i: usize| [r[i], r[4 + i], r[8 + i]];
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            for i in 0..3 {
                for j in 0..3 {
                    let want = if i == j { 1.0 } else { 0.0 };
                    assert!((dot(row(i), row(j)) - want).abs() < 1e-12, "tilt {polar}, heading {azimuth}: not a rotation");
                }
            }
            let back = row(2);
            let forward = look_dir(polar, azimuth);
            assert!(
                (0..3).all(|k| (forward[k] + back[k]).abs() < 1e-12),
                "tilt {polar}, heading {azimuth}: looks along {:?}, not {forward:?}",
                back.map(|v| -v)
            );
        }
    }
}

/// At either pole every heading names the one direction — spherical
/// coordinates' own degeneracy — and the camera's turn there is still a
/// rotation, near the poles and on them.
#[test]
fn at_a_pole_every_heading_looks_the_same_way() {
    let pi = core::f64::consts::PI;
    for theta in [0.0, pi] {
        let first = look_dir(theta, 0.0);
        for phi in [0.7, -2.1, 3.0] {
            let d = look_dir(theta, phi);
            assert!((0..3).all(|k| (d[k] - first[k]).abs() < 1e-12), "θ {theta}: φ {phi} looks along {d:?}, not {first:?}");
        }
    }
    for theta in [0.0, 1e-12, 1e-6, pi - 1e-6, pi - 1e-12, pi] {
        for phi in [0.0, 1.3, -2.9] {
            let r = look_rotation(theta, phi);
            assert!(r.iter().all(|v| v.is_finite()), "θ {theta}, φ {phi}: {r:?}");
            let row = |i: usize| [r[i], r[4 + i], r[8 + i]];
            for i in 0..3 {
                let n = row(i).iter().map(|v| v * v).sum::<f64>();
                assert!((n - 1.0).abs() < 1e-12, "θ {theta}, φ {phi}: an axis of length {n}");
            }
        }
    }
}
