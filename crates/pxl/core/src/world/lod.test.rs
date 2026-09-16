// pxl_core/world/lod.test.rs

use super::*;

#[test]
fn the_reference_ladder_hands_off_at_192_and_doubles() {
    let l = Ladder::default();
    assert!((l.thresholds[0] - 192.0).abs() < 1e-9);
    assert_eq!(l.step_at(100.0, 1), 1);
    assert_eq!(l.step_at(300.0, 1), 2);
    assert_eq!(l.step_at(1e6, 1), 64);
    // Already coarser, a margin inside the threshold keeps it.
    assert_eq!(l.step_at(190.0, 2), 2);
    assert_eq!(l.guide(192.0, 1).blend.map(|b| (b.0, b.1)), Some((1, 2)));
    assert_eq!(l.guide(150.0, 1).blend, None);
}

#[test]
fn two_steps_of_pressure_are_one_tier_coarser_everywhere() {
    let l = Ladder::default();
    let mut pressed = l;
    pressed.set_pressure(2);
    for (i, t) in pressed.thresholds.iter().enumerate() {
        assert!((t - l.thresholds[i] / 2.0).abs() < 1e-9);
    }
    // The nearest chunks have no finer tier to lose; past the finest
    // band's pressed edge, one tier.
    assert_eq!(pressed.step_at(50.0, 1), 1);
    for d in [100.0, 300.0, 700.0, 3000.0] {
        let step = l.step_at(d, 1);
        assert_eq!(pressed.step_at(d, 1), (step * 2).min(COARSEST), "at {d}");
    }
    pressed.set_pressure(0);
    assert_eq!(pressed.step_at(300.0, 1), l.step_at(300.0, 1));
}
