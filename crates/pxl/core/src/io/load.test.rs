// pxl_core/io/load.test.rs

use super::*;

#[test]
fn a_rotation_key_halfway_is_the_slerp_and_a_trs_matrix_places_a_point() {
    let ch = Channel {
        joint: 0,
        path: R,
        step: false,
        times: vec![0.0, 1.0],
        values: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, (0.5f32).sqrt(), (0.5f32).sqrt()],
    };
    let mut out = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    sample(&ch, 0.5, &mut out);
    // Halfway to a quarter turn about Z is an eighth.
    let half = (core::f32::consts::PI / 8.0).sin();
    assert!((out[5] - half).abs() < 1e-5);
    let m = mat_trs(&[1.0, 2.0, 3.0, 0.0, 0.0, (0.5f32).sqrt(), (0.5f32).sqrt(), 2.0, 2.0, 2.0]);
    // (1, 0, 0) scaled by 2 and turned a quarter about Z is (0, 2, 0), then moved.
    let x = m[0] + m[12];
    let y = m[1] + m[13];
    assert!((x - 1.0).abs() < 1e-5 && (y - 4.0).abs() < 1e-5, "{x} {y}");
}
