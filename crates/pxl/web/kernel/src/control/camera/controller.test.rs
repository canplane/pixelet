// pxl-kernel/control/camera/controller.test.rs
//
// The camera is an eye and a look, and the map center is derived from it:
//
// - the map center is the eye pushed the radius along the look, the eye
//   itself first person;
// - a change of person leaves the map center where it is;
// - a pinch keeps no debt past a limit;
// - the look's coordinates hold over their whole domain: every tilt a
//   rotation, and at either pole every heading the one direction.

use super::*;
use crate::control::camera::config::camera_cfg;
use crate::global::serial;
use crate::shell::setting::store::CameraCollisionMode;
use crate::world::fixture::place;
use crate::world::frame::coord::canonical_to_local;

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

/// The map center is the character's place, and a change of person moves
/// the camera about it and never it (`take_mode`): third person to first
/// and back, twice, it stays where it was; a move under way in the one is
/// not carried into the other; and a change asked while a tracker holds the
/// pose is not taken until it lets go.
#[test]
fn a_change_of_person_leaves_the_map_center() {
    let _g = serial();
    third_person();
    // A canonical point keeps its fraction in 256ths of a unit, so the first
    // change lands the map center within one of them; every change after
    // keeps it exactly there, so round trips do not drift.
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
    // Under a tracker the change waits, and the base is left as it was.
    controllers().tracked = Some(Tracked {
        world_turn: mat4(),
        world_at: [0.0; 3],
        turn: mat4(),
        at: [0.0; 3],
    });
    assert!(!take_mode(CameraMode::free), "taken under a tracker");
    assert_eq!(controllers().base, CameraMode::orbit, "the base changed under a tracker");
    controllers().tracked = None;
    assert!(take_mode(CameraMode::free), "not taken once the tracker let go");
    third_person();
}

/// A pinch is the fingers' spread against where it began, and keeps no
/// debt: brought back untouched by a limit it asks the distance it began
/// at; driven past one, the share refused is gone, and the first closing of
/// the fingers zooms back out from where the limit held it. The ground has
/// no say here — the collision set to pass through — so the limits are the
/// distance's own.
#[test]
fn a_pinch_keeps_no_debt() {
    let _g = serial();
    third_person();
    let collision = settings_store().camera_collision;
    settings_store().camera_collision = CameraCollisionMode::through;
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
    settings_store().camera_collision = collision;
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
