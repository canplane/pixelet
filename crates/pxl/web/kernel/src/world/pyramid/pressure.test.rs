// pxl-kernel/world/pyramid/pressure.test.rs

use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::host::{set_heap_stat, HeapStat};

static REFUSALS: AtomicU32 = AtomicU32::new(0);

fn heap() -> HeapStat {
    HeapStat {
        range: 100,
        refusals: REFUSALS.load(Ordering::Relaxed),
        ..HeapStat::default()
    }
}

fn start() {
    set_heap_stat(heap);
    REFUSALS.store(0, Ordering::Relaxed);
    *state() = Pressure {
        level: 0,
        at: f64::NEG_INFINITY,
        refused_at: f64::NEG_INFINITY,
        refusals: 0,
        refusals_at_step: 0,
    };
}

fn refuse() {
    REFUSALS.fetch_add(1, Ordering::Relaxed);
}

/// A refusal that giving back decor answered is no step; one it could not
/// answer is, even when it came while the last step's hold was still on.
#[test]
fn a_refusal_the_reclaim_could_not_answer_steps_in_and_one_it_answered_does_not() {
    let _serial = crate::global::serial();
    start();
    refuse();
    pressure_step(0.0, 3);
    assert_eq!(pressure(), 0, "the reclaim gave something back");
    pressure_step(10.0, 0);
    assert_eq!(pressure(), 1, "the refusal still stands and nothing was freed");
    refuse();
    pressure_step(20.0, 0);
    assert_eq!(pressure(), 1, "inside the hold");
    pressure_step(10.0 + STEP_MS, 0);
    assert_eq!(pressure(), 2, "the refusal inside the hold steps once the hold ends");
    pressure_step(20.0 + 2.0 * STEP_MS, 0);
    assert_eq!(pressure(), 2, "no refusal since the last step");
}

/// With no refusal for RELIEF_MS and the heap off its mark, a step is let out.
#[test]
fn a_spell_without_refusals_lets_a_step_out() {
    let _serial = crate::global::serial();
    start();
    refuse();
    pressure_step(0.0, 0);
    assert_eq!(pressure(), 1);
    pressure_step(RELIEF_MS - 1.0, 0);
    assert_eq!(pressure(), 1);
    pressure_step(RELIEF_MS, 0);
    assert_eq!(pressure(), 0);
}
