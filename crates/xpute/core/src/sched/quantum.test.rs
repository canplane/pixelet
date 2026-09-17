// xpute_core/sched/quantum.test.rs

use super::*;

const POLICY: QuantumPolicy = QuantumPolicy {
    margin_share: 0.15,
    batch_frames: 2.0,
    settle_ms: 150.0,
};

/// A whole window of turns on what each grant said, so what came before is
/// gone; the grant after them.
fn fill(q: &mut Quantum, delta_s: f64, turn_ms: f64, wake_ms: f64) -> Grant {
    for _ in 0..FRAME_WINDOW {
        let grant = q.grant(0.0);
        q.observe(Turn { delta_s, turn_ms, grant, wake_ms });
    }
    q.grant(0.0)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn quantum_the_quota_follows_the_display_s_measured_frame_not_an_assumed_60_hz() {
    let mut q = Quantum::new(POLICY);
    let at_60 = fill(&mut q, 1.0 / 60.0, 0.0, 0.0);
    let at_165 = fill(&mut q, 1.0 / 165.0, 0.0, 0.0);
    assert!(at_165.quota_ms < at_60.quota_ms);
    assert!(near(at_165.frame_ms, 1000.0 / 165.0));
    assert!(near(at_165.quota_ms, 1000.0 / 165.0 * (1.0 - POLICY.margin_share)));
}

#[test]
fn quantum_a_gap_that_is_not_a_frame_moves_nothing() {
    let mut q = Quantum::new(POLICY);
    let before = fill(&mut q, 1.0 / 165.0, 0.0, 0.0);
    q.observe(Turn {
        delta_s: 3.0,
        grant: before,
        wake_ms: 0.0,
        ..Default::default()
    });
    assert_eq!(q.grant(0.0).frame_ms, before.frame_ms);
}

#[test]
fn quantum_a_still_guest_s_frame_gaps_do_not_teach_the_quantum_a_faster_display() {
    let mut q = Quantum::new(POLICY);
    let moving = fill(&mut q, 1.0 / 165.0, 0.0, 0.0);
    // The host catching up after a long still turn: gaps far shorter than
    // any real refresh interval.
    let still = fill(&mut q, 0.002, 0.0, NO_WAKE);
    assert_eq!(still.frame_ms, moving.frame_ms);
}

#[test]
fn quantum_what_a_turn_overran_comes_off_its_own_next_quota_down_to_nothing() {
    let mut q = Quantum::new(POLICY);
    let full = fill(&mut q, 1.0 / 30.0, 0.0, 0.0);
    q.observe(Turn {
        delta_s: 1.0 / 30.0,
        turn_ms: full.quota_ms + 3.0,
        grant: full,
        wake_ms: 0.0,
    });
    assert!(near(q.grant(0.0).quota_ms, full.quota_ms - 3.0));
    q.observe(Turn {
        delta_s: 1.0 / 30.0,
        turn_ms: 100.0,
        grant: full,
        wake_ms: 0.0,
    });
    assert_eq!(q.grant(0.0).quota_ms, 0.0);
}

#[test]
fn quantum_input_or_a_guest_asking_for_the_next_edge_makes_a_turn_interactive() {
    let mut q = Quantum::new(POLICY);
    let idle = fill(&mut q, 1.0 / 30.0, 0.0, NO_WAKE);
    assert!(!idle.interactive);
    assert!(near(idle.quota_ms, idle.frame_ms * POLICY.batch_frames * (1.0 - POLICY.margin_share)));
    q.input(1000.0);
    assert!(q.grant(1000.0 + POLICY.settle_ms - 1.0).interactive);
    assert!(!q.grant(1000.0 + POLICY.settle_ms).interactive);
    let asked = fill(&mut q, 1.0 / 30.0, 0.0, 0.0);
    assert!(asked.interactive);
    assert!(near(asked.quota_ms, asked.frame_ms * (1.0 - POLICY.margin_share)));
}
