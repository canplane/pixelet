// pxl-kernel/control/camera/free.test.rs
//
// A controller that is not the controller does nothing: the free camera,
// with the orbit the controller, takes no navigation command, reads no
// control and casts no look, however many turns pass and whatever the controls did.

use super::*;
use crate::global::serial;

#[test]
fn the_free_camera_does_nothing_while_it_is_not_the_controller() {
    let _g = serial();
    assert_eq!(active(), Active::Orbit);
    let f = free();
    (f.eye, f.yaw, f.pitch) = ([3.0, -4.0, 50.0], 0.25, 0.5);
    let d = drive();
    (d.navigated, d.toward) = (map_store().navigated, None);

    // The controls move and navigation commands come, as they do under the
    // orbit.
    let controls = camera_store().view_state;
    let mut target = Pvf3::default();
    projection::project(&mut target, 90_000.0, 40_000.0, 0.0);
    map_store().navigate_to(target);
    camera_store().view_state.dist = 900.0;
    camera_store().view_state.azimuth = 2.0;

    for _ in 0..30 {
        assert_eq!(turn(1.0 / 60.0), None);
    }
    let (f, d) = (free(), drive());
    assert_eq!((f.eye, f.yaw, f.pitch), ([3.0, -4.0, 50.0], 0.25, 0.5), "the free camera moved");
    assert_eq!(d.toward, None, "the free camera took a command");
    assert_ne!(d.navigated, map_store().navigated, "the free camera took a navigation command");
    camera_store().view_state = controls;
}

/// The free camera stands in the orbit's space: carried up, down or north
/// past it, it stops no higher than the orbit's eye goes, no lower than the
/// lattice reaches, and within the lattice's rows.
#[test]
fn the_free_camera_stands_in_the_space_the_orbit_does() {
    let _g = serial();
    // Where the free camera's eye is carried to, in units.
    let at = |k: usize| {
        let o = view().origin;
        drive().toward.unwrap()[k] + [ivec::to_real(ivec::X(o)), ivec::to_real(ivec::Y(o)), ivec::to_real(ivec::Z(o))][k]
    };
    let f = free();
    (f.eye, f.yaw, f.pitch) = ([0.0, 0.0, 100.0], 0.0, 0.5);
    drive().toward = None;
    rise(1.0e9 / UNITS_PER_PX);
    assert_eq!(at(2), MAX_DIST, "carried up");
    rise(-1.0e9 / UNITS_PER_PX);
    assert_eq!(at(2), REACH.min[2], "carried down");
    drive().toward = None;
    free().eye[1] = 1.0e9;
    rise(0.0);
    assert_eq!(at(1), REACH.max[1] - 1.0, "carried north");
}

/// A look tilts over the whole of its domain: at every tilt, straight down
/// to straight up, the camera's turn is a rotation, it looks the way its
/// heading and tilt say, and nothing in it comes apart.
#[test]
fn a_look_tilts_over_its_whole_domain() {
    use crate::control::camera::orbit::look_rotation;
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

/// The free camera moves as far whichever way it goes and wherever it
/// stands: a hundred pixels of drag across and of zoom up carry the eye one
/// distance, low or high, looking down or up.
#[test]
fn the_free_camera_moves_one_distance_every_way_and_everywhere() {
    let _g = serial();
    // Where the eye is carried to: the eye itself at rest.
    let at = || {
        let o = view().origin;
        let e = drive().toward.unwrap_or(free().eye);
        [e[0] + ivec::to_real(ivec::X(o)), e[1] + ivec::to_real(ivec::Y(o)), e[2] + ivec::to_real(ivec::Z(o))]
    };
    let moves: [fn(); 2] = [|| pan(100.0, 0.0), || rise(100.0)];
    let mut first = None;
    for (z, pitch) in [(400.0, 0.8), (40_000.0, 2.6), (2_000.0, 0.0)] {
        for m in moves {
            let f = free();
            (f.eye, f.yaw, f.pitch) = ([0.0, 0.0, z], 0.3, pitch);
            drive().toward = None;
            view_compose();
            let a = at();
            m();
            let b = at();
            let by = (b[0] - a[0]).hypot(b[1] - a[1]).hypot(b[2] - a[2]);
            let first = *first.get_or_insert(by);
            assert!(by > 0.0 && (by - first).abs() <= 1e-9 * first, "at z {z}, pitch {pitch}: {by}, not {first}");
        }
    }
}

/// At either pole every heading names the one direction — spherical
/// coordinates' own degeneracy — and the camera's turn there is still a
/// rotation, near the poles and on them.
#[test]
fn at_a_pole_every_heading_looks_the_same_way() {
    use crate::control::camera::orbit::look_rotation;
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
