// pxl_render/look/lod.test.rs

use super::*;
use crate::serial;

// These are explicit world-space thresholds. They must not move when L2's
// ownership/remesh size changes.
const THRESHOLD_0: f64 = 192.0;
const THRESHOLD_1: f64 = 384.0;
const THRESHOLD_2: f64 = 768.0; // LOD2<->LOD3 handoff
const THRESHOLD_3: f64 = 1536.0; // LOD3<->LOD4 handoff

#[test]
fn lod_step_at_the_ladder_follows_the_projection_twice_the_viewport_height_twice_the_handoff_distance() {
    let _g = serial();
    // The default is the reference projection, where the first handoff is 192u.
    assert_eq!(lod_step_at(300.0, 1), 2);
    set_lod_projection(2.0 * 1080.0, 50.0, 0);
    let wider = lod_step_at(300.0, 1);
    set_lod_projection(1080.0, 50.0, 0);
    assert_eq!(wider, 1);
    assert_eq!(lod_step_at(300.0, 1), 2);
}

#[test]
fn lod_step_at_two_steps_of_the_module_s_pressure_are_one_rung_coarser() {
    let _g = serial();
    set_lod_projection(1080.0, 50.0, 2);
    let (near, far) = (lod_step_at(150.0, 1), lod_step_at(300.0, 1));
    set_lod_projection(1080.0, 50.0, 0);
    assert_eq!(near, 2);
    assert_eq!(far, 4);
}

#[test]
fn lod_step_at_the_rungs_are_the_module_s_192_384_768_1536_at_the_reference_64_past_the_last() {
    let _g = serial();
    set_lod_projection(1080.0, 50.0, 0);
    assert_eq!(lod_step_at(THRESHOLD_0 - 1.0, 1), 1);
    assert_eq!(lod_step_at(THRESHOLD_0 + 1.0, 1), 2);
    assert_eq!(lod_step_at(THRESHOLD_1 + 1.0, 1), 4);
    assert_eq!(lod_step_at(THRESHOLD_2 + 1.0, 1), 8);
    assert_eq!(lod_step_at(THRESHOLD_3 + 1.0, 1), 16);
    assert_eq!(lod_step_at(1_000_000.0, 1), 64);
}

#[test]
fn lod_step_at_a_badge_climbs_the_cube_ladder_same_handoff_same_hysteresis_the_badge_tier_a_fixed_shift_above() {
    let _g = serial();
    set_lod_projection(1080.0, 50.0, 0);
    // Just past the first handoff both go coarse together.
    let past = lod_step_at(200.0, 1);
    assert_eq!(past, 2);
    // Back just inside it, the coarser tier holds — the hysteresis a chunk has.
    assert_eq!(lod_step_at(190.0, past), 2);
    // Well inside, both come back.
    assert_eq!(lod_step_at(150.0, past), 1);
    assert_eq!(get_lod_level(lod_step_at(200.0, 1)) + BADGE_SHAMT_OVER_CUBE, 3);
}

#[test]
fn the_reference_ladder_hands_off_at_192_and_doubles() {
    let l = Ladder::default();
    assert!((l.threshold_0() - THRESHOLD_0).abs() < 1e-9);
    assert_eq!(l.step_at(100.0, 1), 1);
    assert_eq!(l.step_at(300.0, 1), 2);
    assert_eq!(l.step_at(1e6, 1), 64);
    // Already coarser, a margin inside the threshold keeps it.
    assert_eq!(l.step_at(190.0, 2), 2);
    assert_eq!(l.guide(THRESHOLD_0, 1).blend.map(|b| (b.0, b.1)), Some((1, 2)));
    assert_eq!(l.guide(150.0, 1).blend, None);
}

#[test]
fn two_steps_of_pressure_are_one_tier_coarser_everywhere() {
    let l = Ladder::default();
    let mut pressed = l;
    pressed.set_pressure(2);
    assert!((pressed.threshold_0() - l.threshold_0() / 2.0).abs() < 1e-9);
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
