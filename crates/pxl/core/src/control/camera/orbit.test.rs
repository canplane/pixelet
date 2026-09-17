// pxl_core/control/camera/orbit.test.rs

use super::*;

/// `assertAlmostEquals`' default tolerance.
fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} is not {b}");
}

#[test]
fn orbit_local_offset_polar_0_looks_straight_down_all_offset_is_vertical() {
    let o = orbit_local_offset(100.0, 0.0);
    close(o.horiz_dist, 0.0, 1e-7);
    close(o.vert_dist, 100.0, 1e-7);
}

#[test]
fn orbit_local_offset_polar_pi_2_looks_at_the_horizon_all_offset_is_horizontal() {
    let o = orbit_local_offset(100.0, core::f64::consts::PI / 2.0);
    close(o.horiz_dist, 100.0, 1e-7);
    close(o.vert_dist, 0.0, 1e-9);
}

#[test]
fn orbit_local_offset_offset_magnitude_always_equals_dist_regardless_of_polar() {
    for polar in [0.0, 0.3, 0.7, 1.0, 1.5] {
        let o = orbit_local_offset(50.0, polar);
        close(o.horiz_dist.hypot(o.vert_dist), 50.0, 1e-7);
    }
}

#[test]
fn rotate2_identity_at_angle_0() {
    let r = rotate2(3.0, 4.0, 0.0);
    close(r.x, 3.0, 1e-7);
    close(r.y, 4.0, 1e-7);
}

#[test]
fn rotate2_quarter_turn_maps_x_onto_y() {
    let r = rotate2(1.0, 0.0, core::f64::consts::PI / 2.0);
    close(r.x, 0.0, 1e-9);
    close(r.y, 1.0, 1e-9);
}

#[test]
fn rotate2_preserves_vector_length_pure_rotation_no_scaling() {
    let r = rotate2(3.0, -7.0, 1.234);
    close(r.x.hypot(r.y), 3f64.hypot(-7.0), 1e-7);
}

#[test]
fn rotate2_rotating_by_angle_undoes_rotating_by_angle() {
    let angle = 0.9;
    let forward = rotate2(5.0, -2.0, angle);
    let back = rotate2(forward.x, forward.y, -angle);
    close(back.x, 5.0, 1e-7);
    close(back.y, -2.0, 1e-7);
}

#[test]
fn damp_alpha_zero_elapsed_time_never_moves_the_damped_value() {
    assert_eq!(damp_alpha(10.0, 0.0), 0.0);
}

#[test]
fn damp_alpha_grows_toward_1_as_delta_grows_without_overshooting_it() {
    let a = damp_alpha(10.0, 0.05);
    let b = damp_alpha(10.0, 1.0);
    assert!(a < b);
    assert!(b < 1.0);
}

#[test]
fn damp_alpha_underflows_to_exactly_1_once_delta_is_large_enough() {
    assert_eq!(damp_alpha(10.0, 100.0), 1.0);
}

#[test]
fn damp_alpha_a_larger_rate_reaches_the_same_alpha_in_less_time() {
    assert!(damp_alpha(20.0, 0.1) > damp_alpha(5.0, 0.1));
}
