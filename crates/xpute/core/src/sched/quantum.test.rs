// xpute_core/sched/quantum.test.rs

use super::*;

const POLICY: QuantumPolicy = QuantumPolicy {
    margin_share: 0.15,
    min_ms: 2.0,
    max_ms: 8.0,
    batch_ms: 12.0,
    outside_ema: 0.1,
};

/// A whole window of turns, so what came before is gone.
fn fill(q: &mut Quantum, delta_s: f64, outside_ms: f64, interactive: bool) -> Grant {
    for _ in 0..FRAME_WINDOW {
        q.observe(Turn {
            delta_s,
            turn_ms: outside_ms,
            used_ms: 0.0,
            interactive,
        });
    }
    q.grant()
}

#[test]
fn quantum_the_slice_follows_the_display_s_measured_frame_not_an_assumed_60_hz() {
    let mut q = Quantum::new(POLICY);
    let at_60 = fill(&mut q, 1.0 / 60.0, 0.0, true);
    let at_165 = fill(&mut q, 1.0 / 165.0, 0.0, true);
    assert!(at_165.slice_ms < at_60.slice_ms, "165 Hz got {}, 60 Hz got {}", at_165.slice_ms, at_60.slice_ms);
    assert!((at_165.frame_ms - 1000.0 / 165.0).abs() < 1e-9);
}

#[test]
fn quantum_a_gap_that_is_not_a_frame_moves_nothing() {
    let mut q = Quantum::new(POLICY);
    let before = fill(&mut q, 1.0 / 165.0, 0.0, true);
    q.observe(Turn {
        delta_s: 3.0,
        interactive: true,
        ..Default::default()
    });
    assert_eq!(q.grant().frame_ms, before.frame_ms);
}

#[test]
fn quantum_a_still_guest_s_frame_gaps_do_not_teach_the_quantum_a_faster_display() {
    let mut q = Quantum::new(POLICY);
    let moving = fill(&mut q, 1.0 / 165.0, 0.0, true);
    // The host catching up after a long still turn: gaps far shorter than
    // any real refresh interval.
    let still = fill(&mut q, 0.002, 0.0, false);
    assert_eq!(still.frame_ms, moving.frame_ms);
}

#[test]
fn quantum_what_a_turn_costs_outside_the_slice_comes_off_the_next_down_to_the_floor() {
    let mut q = Quantum::new(POLICY);
    let light = fill(&mut q, 1.0 / 60.0, 10.0, true).slice_ms;
    let frame = 1000.0 / 60.0;
    assert!((light - (frame - frame * POLICY.margin_share - 10.0)).abs() < 0.01, "slice {light}");
    let heavy = fill(&mut q, 1.0 / 60.0, 30.0, true).slice_ms;
    assert_eq!(heavy, POLICY.min_ms);
}

#[test]
fn quantum_a_cheap_turn_is_granted_no_more_than_the_ceiling_and_the_batch_quantum_beside_it() {
    let mut q = Quantum::new(POLICY);
    let g = fill(&mut q, 1.0 / 30.0, 0.0, true);
    assert_eq!(g.slice_ms, POLICY.max_ms);
    assert_eq!(g.batch_ms, POLICY.batch_ms);
}
