// pxl-render/look/lod.test.rs

use super::*;

// An explicit world-space threshold. It must not move when L2's
// ownership/remesh size changes.
const THRESHOLD_0: f64 = 192.0;

#[test]
fn the_first_handoff_is_192_at_the_reference_and_follows_the_projection() {
    assert!((Ladder::default().threshold_0() - THRESHOLD_0).abs() < 1e-9);
    // Twice the viewport's height, twice the pixels a cell covers, so twice
    // the distance before it is as small as the reference's.
    assert!((Ladder::new(2.0 * REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG, 0).threshold_0() - 2.0 * THRESHOLD_0).abs() < 1e-9);
}

#[test]
fn two_steps_of_pressure_halve_the_first_handoff() {
    let l = Ladder::default();
    let mut pressed = l;
    pressed.set_pressure(2);
    assert!((pressed.threshold_0() - l.threshold_0() / 2.0).abs() < 1e-9);
    pressed.set_pressure(0);
    assert_eq!(pressed.threshold_0(), l.threshold_0());
}
