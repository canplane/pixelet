// pxl-kernel/world/frame/resolution.test.rs

use super::*;

const BUDGET: f64 = 1000.0 / 60.0;

/// A fresh controller under `mode`.
fn start(mode: ResolutionMode) {
    settings_store().resolution = mode;
    *resolution() = Resolution {
        ratio: 0.0,
        gpu_ms: None,
        over_ms: 0.0,
        room_ms: 0.0,
        since_change_ms: 0.0,
        probe_hold_ms: PROBE_HOLD_MS,
        probing: false,
    };
}

/// `seconds` of moving frames on a device of ratio `dpr`, each taking
/// `gap_ms` and, where given, `gpu_ms` of the GPU; the ratio after.
fn run(dpr: f64, seconds: f64, gap_ms: f64, gpu_ms: Option<f64>) -> f64 {
    let state = TickState {
        dpr,
        frame_ms: BUDGET,
        interactive: true,
        gap_s: gap_ms / 1000.0,
        ..TickState::default()
    };
    let mut ratio = 0.0;
    for _ in 0..(seconds * 1000.0 / gap_ms) as usize {
        resolution_observe(gpu_ms.unwrap_or(0.0));
        ratio = resolution_step(&state);
        assert!((DPR_MIN..=dpr.clamp(DPR_MIN, DPR_MAX)).contains(&ratio), "the ratio {ratio} left its range");
    }
    ratio
}

#[test]
fn fixed_is_the_devices_ratio_and_low_is_one_whatever_the_frames_cost() {
    let _serial = crate::global::serial();
    start(ResolutionMode::fixed);
    assert_eq!(run(3.0, 3.0, 50.0, Some(40.0)), DPR_MAX);
    start(ResolutionMode::low);
    assert_eq!(run(2.0, 3.0, BUDGET, Some(1.0)), DPR_MIN);
}

#[test]
fn auto_steps_down_under_load_and_back_up_once_the_gpu_has_room() {
    let _serial = crate::global::serial();
    start(ResolutionMode::auto);
    // The GPU over its budget: a step down a second, to the floor.
    assert_eq!(run(2.0, 6.0, 2.0 * BUDGET, Some(1.2 * BUDGET)), DPR_MIN);
    // Room under half the budget: back up a step at a time, to the device's.
    assert_eq!(run(2.0, 15.0, BUDGET, Some(0.3 * BUDGET)), 2.0);
}

/// A display faster than sixty frames a second is held to sixty: GPU time
/// past its own interval but inside a sixtieth keeps the device's ratio.
#[test]
fn a_fast_display_is_held_to_sixty_frames_and_no_more() {
    let _serial = crate::global::serial();
    start(ResolutionMode::auto);
    let state = TickState {
        dpr: 2.0,
        frame_ms: 1000.0 / 120.0,
        interactive: true,
        gap_s: 1.0 / 120.0,
        ..TickState::default()
    };
    for _ in 0..600 {
        resolution_observe(12.0);
        resolution_step(&state);
    }
    assert_eq!(resolution().ratio, 2.0);
}

#[test]
fn a_blind_probe_the_frames_cannot_carry_makes_the_next_wait_twice_as_long() {
    let _serial = crate::global::serial();
    start(ResolutionMode::auto);
    // No GPU time: missed frames take it down.
    let low = run(2.0, 3.0, 2.0 * BUDGET, None);
    assert!(low < 2.0);
    // Frames on budget: a probe up after PROBE_HOLD_MS.
    let up = run(2.0, PROBE_HOLD_MS / 1000.0 + 0.1, BUDGET, None);
    assert_eq!(up, low + STEP);
    // Missed again at once: it comes back down, and the next probe waits twice as long.
    assert_eq!(run(2.0, 1.1, 2.0 * BUDGET, None), low);
    assert_eq!(resolution().probe_hold_ms, 2.0 * PROBE_HOLD_MS);
}
