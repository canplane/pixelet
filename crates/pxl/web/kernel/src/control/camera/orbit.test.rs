// pxl-kernel/control/camera/orbit.test.rs
//
// What the turn observes is the view, and the view stands where the map
// center is by the time anything observes it where the map center jumped:
//
// - an orbit not placed yet is put at the map center the turn first has,
//   every control at once;
// - a jump of the map center puts the view there before the turn observes;
// - a move within the jump distance is left to the tick's damping, so what
//   is observed is the view being drawn and not the control's target.

use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use super::*;
use crate::global::serial;

fn at(x: f64, y: f64) -> Pvf3 {
    let mut p = Pvf3::default();
    projection::project(&mut p, x, y, 12.0);
    p
}

/// The orbit's pivot, in units.
fn center() -> [f64; 3] {
    let mut c = Pvf3::default();
    crate::world::frame::coord::local_world_to_canonical(&mut c, &orbit().pivot, view().origin);
    [pvf3::to_real(pvf3::X(&c)), pvf3::to_real(pvf3::Y(&c)), pvf3::to_real(pvf3::Z(&c))]
}

fn is_at(c: [f64; 3], x: f64, y: f64) -> bool {
    (c[0] - x).abs() < 1e-6 && (c[1] - y).abs() < 1e-6 && (c[2] - 12.0).abs() < 1e-6
}

#[test]
fn an_orbit_not_placed_yet_is_put_at_the_first_map_center_whole() {
    let _g = serial();
    orbit().placed = false;
    map_store().map_center = Some(at(5_911_552.5, 1_900_544.25));
    crate::control::camera::controller::snap_step();
    assert!(is_at(center(), 5_911_552.5, 1_900_544.25), "{:?}", center());
    let (o, vs) = (orbit(), camera_store().view_state);
    assert!(o.placed);
    assert_eq!((o.dist, o.polar, o.climb, o.azimuth), (vs.dist, vs.polar, 0.0, vs.azimuth), "placed by halves");
}

#[test]
fn a_jump_of_the_map_center_puts_the_view_there_before_the_turn_observes() {
    let _g = serial();
    map_store().map_center = Some(at(1_000.0, 2_000.0));
    crate::control::camera::controller::snap_step();
    let far = camera_store().view_state.dist * JUMP_DISTANCES * 3.0;
    map_store().map_center = Some(at(1_000.0 + far, 2_000.0 - far));
    crate::control::camera::controller::snap_step();
    assert!(is_at(center(), 1_000.0 + far, 2_000.0 - far), "{:?}", center());
}

#[test]
fn a_move_within_the_jump_distance_is_left_to_the_damping() {
    let _g = serial();
    map_store().map_center = Some(at(1_000.0, 2_000.0));
    crate::control::camera::controller::snap_step();
    let near = camera_store().view_state.dist * JUMP_DISTANCES * 0.5;
    map_store().map_center = Some(at(1_000.0 + near, 2_000.0));
    crate::control::camera::controller::snap_step();
    assert!(is_at(center(), 1_000.0, 2_000.0), "{:?}", center());
}
