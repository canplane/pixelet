// xpute-host/native/pause.test.rs

// A binary of its own: a process boots one guest, and these tests count its
// turns.
#![cfg(unix)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use xpute_guest::mem::section::PAGE_BYTES;
use xpute_host::native::{self, Config};

static TURNS: AtomicU32 = AtomicU32::new(0);

/// A turn that asks for the next at once, as one still at work does: it runs
/// at the next kick, as a page's runs at its strobe's next moment.
fn turn(_: f64) -> f64 {
    TURNS.fetch_add(1, Ordering::SeqCst);
    0.0
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
            data_end: 0,
            heap_at: 16 * PAGE_BYTES,
            stack_bytes: 1 << 20,
            thread_name: "pause-guest",
            clock: || 0.0,
            stop_hook: None,
            admit: None,
            publish: None,
            ready: || true,
        },
        turn,
    );
    settle();
    assert_eq!(turns(), 0, "paused from boot, nothing runs");

    native::kick_quota(-1.0);
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
        native::ack().0,
        0.0,
        "and the program is told the guest wants it at once"
    );

    native::kick_quota(-1.0);
    settle();
    assert_eq!(turns(), 2, "the kick runs it, and nothing after it");

    native::pause();
    settle();
    let paused_at = turns();
    settle();
    assert!(
        turns() <= paused_at + 1,
        "paused again, at most the turn under way ends"
    );
}
