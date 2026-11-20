// pxl-core/domain/control/camera/controller.test.rs

use super::*;
use crate::domain::control::camera::fixture::{camera_now, place};
use crate::domain::control::param;
use crate::domain::control::param::{DIST_DAMP, FLIGHT};
use crate::domain::eye;
use crate::domain::layer::object::store::Store;
use crate::domain::query::test::{flat_at_five, placed as ground};
use crate::domain::query::{eye_over, over};
use crate::domain::smooth::smoothing_factor;
use crate::domain::view::frame::pose::view_resize;
use pxl_spec::space::coord::world;

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

fn orbit_mode() {
    set_mode(CameraMode::Orbit);
}

/// The pivot lies the orbit's distance ahead of the eye for the orbit, and
/// under the eye in first person, by the eye's height over where a body
/// stands.
#[test]
fn pivot_along_facing() {
    orbit_mode();
    let mut rng = Rng(0x5eed);
    for n in 0..200 {
        let at = placed(&mut rng);
        assert!(near(camera_now().pivot, at, 1e-9), "{n}: the orbit");
        set_mode(CameraMode::FirstPerson);
        // The pivot stands over a floor as a body does, and the eye over the
        // same floor as the eye in first person does.
        let [px, py, pz] = camera_now().pivot;
        let floor = pz - over(0.0);
        assert!(
            near([px, py, eye_over(floor)], pose().unwrap().eye, 1e-12),
            "{n}: first person"
        );
        orbit_mode();
    }
}

/// A change of camera mode moves the eye about the pivot and never moves the
/// pivot: the camera is the same one, whichever controller draws its eye.
#[test]
fn a_mode_change_keeps_the_pivot() {
    orbit_mode();
    let mut rng = Rng(0xc0de);
    for n in 0..100 {
        placed(&mut rng);
        let was = camera_now();
        for mode in [
            CameraMode::FirstPerson,
            CameraMode::Orbit,
            CameraMode::FirstPerson,
            CameraMode::Orbit,
        ] {
            set_mode(mode);
            assert_eq!(camera_now(), was, "{n}: redrawn by {mode:?}");
        }
    }
}

/// A move ends where what it has left cannot be told apart in the view: the
/// camera then stands on its end whole, on no move. It ends whichever side
/// the pivot comes from, at an azimuth where the eye's target lies on the
/// lattice itself.
#[test]
fn a_move_ends_where_what_is_left_cannot_be_told() {
    view_resize(1280.0, 720.0);
    orbit_mode();
    for (to, azimuth) in [([8.0, 3.0, 10.0], 0.0), ([-8.0, -3.0, 10.0], 0.5)] {
        place([0.0, 0.0, 10.0], 400.0, 1.0, 0.0);
        end_as(|m| Move {
            to,
            azimuth,
            dist: 300.0,
            ..m
        });
        let rested = (0..10_000).find(|_| !step(1.0 / 60.0));
        assert!(rested.is_some(), "the move toward {to:?} ends");
        assert_eq!(*motion(), None, "and the camera is on no move");
        assert_eq!((camera_now().azimuth, camera_now().dist), (azimuth, 300.0));
    }
}

/// The orbit flies to a point it is told to go to, level to one beside it,
/// at its own distance. First person is eased to a point where the eye
/// draws the ground as it is and warps to one farther.
#[test]
fn the_orbit_flies_and_first_person_is_eased_or_warps() {
    view_resize(1280.0, 720.0);
    let [x, y] = flat_at_five();
    ground(Store::default());
    orbit_mode();
    let home = [x + 0.5, y + 0.5, over(5.0)];
    let near = world::point(x + 8.5, y + 0.5, over(5.0));
    let far = world::point(x + 3000.5, y + 0.5, over(5.0));
    place(home, DEFAULT_DIST, 1.0, 0.0);
    eye::set_pose(pose());
    let eye_z = || pose().unwrap().eye[2];
    let level = eye_z();
    go(near);
    let mut highest = 0.0f64;
    let rested = (0..600).find(|_| {
        highest = highest.max(eye_z() - level);
        assert_eq!(camera_now().dist, DEFAULT_DIST, "the distance is its own");
        !step(1.0 / 60.0)
    });
    assert!(rested.is_some(), "the near flight ends");
    // The flight's top: the view as wide as it began and curve² / 2 of the
    // way, squared and summed, and the camera carried by what that adds.
    let top = libm::hypot(DEFAULT_DIST, FLIGHT.curve * FLIGHT.curve / 2.0 * 8.0) - DEFAULT_DIST;
    assert!((highest - top).abs() < 1e-3, "{highest} against {top}");
    assert!(top < DEFAULT_DIST * 0.01, "which is level");
    go(far);
    assert!((0..20).all(|_| step(1.0 / 60.0)));
    assert!(eye_z() - level > DEFAULT_DIST, "carried high: {}", eye_z());
    // First person's eye never leaves its body: a change of mode on a
    // flight leaves no height to come back to.
    set_mode(CameraMode::FirstPerson);
    assert!(step(1.0 / 60.0));
    orbit_mode();
    assert!(eye_z() - level < DEFAULT_DIST, "down: {}", eye_z());
    let rested = (0..2000).find(|_| !step(1.0 / 60.0));
    assert!(rested.is_some(), "the flight ends");
    assert!((camera_now().pivot[0] - (x + 3000.5)).abs() < 1e-6, "there");

    place(home, DEFAULT_DIST, 1.0, 0.0);
    set_mode(CameraMode::FirstPerson);
    eye::set_pose(pose());
    go(near);
    assert!(step(1.0 / 60.0), "it is eased");
    let walked = camera_now().pivot[0];
    assert!(walked > home[0] && walked < x + 8.5, "a step of the way");
    go(far);
    assert_eq!(camera_now().pivot[0], x + 3000.5, "it warps: there at once");
    assert_eq!(
        motion().map(|m| m.to),
        Some(to_world(&far)),
        "the move ends there"
    );
    eye::set_pose(pose());
    assert!(step(1.0 / 60.0), "the frame its move ends");
    assert!(!step(1.0 / 60.0), "and nothing moves after");
    orbit_mode();
}

/// A zoom is eased at its own rate, and a pan beside it moves the pivot at
/// once, by its own length, in reals: pans far shorter than a point's step
/// add up to their sum, either way. Told to stay, the camera stays as it
/// is, and comes down in place from a flight.
#[test]
fn a_pan_is_at_once_and_the_rest_is_eased() {
    view_resize(1280.0, 720.0);
    orbit_mode();
    let delta = 1.0 / 60.0;
    place([0.5, 0.5, 10.0], 64.0, 1.0, 0.0);
    eye::set_pose(pose());
    zoom(4.0);
    assert!(step(delta));
    let eased = 64.0 + (16.0 - 64.0) * smoothing_factor(DIST_DAMP, delta);
    assert!(
        (camera_now().dist - eased).abs() < 1e-9,
        "eased at its rate"
    );
    let was = camera_now().pivot;
    pan_by(2.0, 0.0);
    let moved = libm::hypot(
        camera_now().pivot[0] - was[0],
        camera_now().pivot[1] - was[1],
    );
    assert!((moved - 2.0).abs() < 1e-9, "at once: {moved}");
    assert_eq!(motion().map(|m| m.dist), Some(16.0), "the zoom eases on");
    let was = camera_now().pivot;
    for way in [1.0, -1.0] {
        for _ in 0..1000 {
            pan_by(way * 1e-4, 0.0);
        }
        let moved = libm::hypot(
            camera_now().pivot[0] - was[0],
            camera_now().pivot[1] - was[1],
        );
        let want = if way > 0.0 { 0.1 } else { 0.0 };
        assert!((moved - want).abs() < 1e-9, "{way}: {moved}");
    }
    let rested = (0..3000).find(|_| !step(delta));
    assert!(
        rested.is_some() && camera_now().dist == 16.0,
        "the ease ends"
    );

    go(world::point(4000.5, 0.5, 10.0));
    assert!((0..30).all(|_| step(delta)));
    let high = pose().unwrap().eye[2];
    let at = camera_now();
    zoom(2.0);
    stay();
    assert_eq!(
        motion().map(|m| (m.to, m.dist)),
        Some((at.pivot, at.dist / 2.0)),
        "the pivot stays, and the zoom asked is eased on"
    );
    let rested = (0..3000).find(|_| !step(delta));
    assert!(rested.is_some(), "it comes down");
    assert_eq!(camera_now().pivot, at.pivot, "where it was");
    assert!(pose().unwrap().eye[2] < high, "from the flight's height");
}
