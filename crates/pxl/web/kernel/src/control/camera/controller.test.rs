// pxl-kernel/control/camera/controller.test.rs

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

fn third_person() {
    tracked_end();
    controllers().base = CameraMode::orbit;
    settings_store().camera_mode = CameraMode::orbit;
}

/// The orbit's distance third person, none first.
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

/// A change of person moves the camera about the map center, never the
/// center; under a tracker it waits until the tracker lets go.
#[test]
fn a_turn_keeps_the_point_it_turns_about() {
    let _g = serial();
    third_person();
    // A canonical point keeps 256ths of a unit: the first change lands within
    // one, and every later change keeps it exactly there.
    let near = |a: V3, b: V3, tol: f64| (0..3).all(|k| (a[k] - b[k]).abs() <= tol);
    let mut rng = Rng(0xc0de);
    for n in 0..100 {
        let center = [rng.unit() * 2_000.0 - 1_000.0, rng.unit() * 2_000.0 - 1_000.0, rng.unit() * 40.0];
        let (dist, polar, azimuth) = (camera_cfg::view::MIN_DIST + rng.unit() * 2_000.0, rng.unit() * core::f64::consts::PI, rng.unit() * 6.0 - 3.0);
        place(center, dist, polar, azimuth);
        let mut first: Option<V3> = None;
        for mode in [CameraMode::free, CameraMode::orbit, CameraMode::free, CameraMode::orbit] {
            assert!(take_mode(mode), "{n}: the change to {mode:?} taken");
            let got = center_local().unwrap();
            assert!(near(got, center, 2.0 / 256.0), "{n}: moved by the change to {mode:?}: {got:?} vs {center:?}");
            let first = *first.get_or_insert(got);
            assert!(near(got, first, 1e-6), "{n}: drifted by the change to {mode:?}: {got:?} vs {first:?}");
            let target = match mode {
                CameraMode::free => crate::control::camera::free::target_center(),
                CameraMode::orbit => crate::control::camera::orbit::target_center(),
            };
            let target = canonical_to_local(&target.unwrap(), view().origin);
            assert!(near(target, got, 1e-6), "{n}: a move under way carried into {mode:?}");
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

/// Collision set to pass through, so the limits are the distance's own.
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

/// Spherical coordinates' own degeneracy: the turn is still a rotation there.
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
