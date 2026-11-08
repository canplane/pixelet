// xpute-runtime/native/pause.test.rs

// This test is a binary of its own, because a process boots one core and these
// tests count that core's turns.
#![cfg(unix)]

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use xpute_core::mem::section::PAGE_BYTES;
use xpute_runtime::native::{self, Config};

static TURNS: AtomicU32 = AtomicU32::new(0);
/// The wake each turn answers, in ms, kept as the bits of an f64; 0 at first.
static WAKE: AtomicU64 = AtomicU64::new(0);

/// A turn that asks to be woken in `WAKE` ms, where 0 means at once. The next
/// turn still runs only at the next kick, as a page's turn runs at its
/// scheduler's moment.
fn turn(_: f64) -> f64 {
    TURNS.fetch_add(1, Ordering::SeqCst);
    f64::from_bits(WAKE.load(Ordering::SeqCst))
}

fn turns() -> u32 {
    TURNS.load(Ordering::SeqCst)
}

/// Long enough for a thread that would run a turn to have run it.
fn settle() {
    std::thread::sleep(Duration::from_millis(50));
}

#[test]
fn paused_runs_only_the_rung_turn() {
    native::pause();
    native::boot(
        &Config {
            mem_bytes: 4 << 20,
            heap_at: 16 * PAGE_BYTES,
            stack_bytes: 1 << 20,
            thread_name: "pause-core",
            ready: || true,
            form: xpute_runtime::native::Form {
                data_end: 0,
                now: || 0.0,
                stop_hook: None,
                admit_frame: None,
                publish: None,
            },
        },
        turn,
    );
    settle();
    assert_eq!(turns(), 0, "paused from boot, nothing runs");

    native::kick_deadline(-1.0);
    settle();
    assert_eq!(turns(), 0, "a kick waits for resume");

    assert_eq!(native::ring(0.0), Some(0.0));
    settle();
    assert_eq!(
        turns(),
        1,
        "one ring, one turn, though it asked for the next"
    );

    native::resume();
    settle();
    assert_eq!(
        turns(),
        1,
        "resumed, a turn that asked for the next at once waits for a paced kick"
    );
    assert_eq!(
        native::turns().0,
        0.0,
        "and the program is told the core wants it at once"
    );

    native::kick_deadline(-1.0);
    settle();
    assert_eq!(turns(), 2, "the kick runs it, and nothing after it");

    WAKE.store(10.0_f64.to_bits(), Ordering::SeqCst);
    let _ = native::turns();
    native::kick_deadline(-1.0);
    settle();
    let (wake, _) = native::turns();
    assert!(
        (0.0..=10.0).contains(&wake),
        "a turn's timed wake is told the program, {wake} ms"
    );
    settle();
    assert_eq!(turns(), 3, "and its time passes with no turn of its own");

    native::pause();
    settle();
    let paused_at = turns();
    settle();
    assert!(
        turns() <= paused_at + 1,
        "paused again, at most the turn under way ends"
    );
}
