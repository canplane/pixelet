// xpute-runtime/native/pause.test.rs

// This test is a binary of its own, because a process boots one core and these
// tests count that core's turns.
#![cfg(unix)]

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use xpute_core::mem::section::PAGE_BYTES;
use xpute_runtime::native::{self, Config, Core};

static TURNS: AtomicU32 = AtomicU32::new(0);
/// The wake each turn answers, in ms, kept as the bits of an f64; 0 at first.
static WAKE: AtomicU64 = AtomicU64::new(0);

/// A turn that takes every frame waiting for it, as a core's door does, and
/// asks to be woken in `WAKE` ms, where 0 means at once. The next turn still
/// runs only at the next display frame.
fn turn(_: f64) -> f64 {
    TURNS.fetch_add(1, Ordering::SeqCst);
    while xpute_core::host::receive(PAGE_BYTES, PAGE_BYTES) > 0 {}
    f64::from_bits(WAKE.load(Ordering::SeqCst))
}

fn config() -> Config {
    Config {
        mem_bytes: 4 << 20,
        heap_at: 16 * PAGE_BYTES,
        stack_bytes: 1 << 20,
        thread_name: "pause-core",
        data_end: 0,
        now: || 0.0,
        stop_hook: None,
        publish: None,
        slow_ms: None,
    }
}

fn turns() -> u32 {
    TURNS.load(Ordering::SeqCst)
}

/// Long enough for a thread that would run a turn to have run it.
fn settle() {
    std::thread::sleep(Duration::from_millis(50));
}

#[test]
fn paused_runs_only_the_turn_asked_for() {
    native::pause();
    static CORE: OnceLock<Core> = OnceLock::new();
    let core = *CORE.get_or_init(|| native::boot(&config(), turn));
    // The core receives through the host its boot hands it, into a page below
    // the heap that nothing else touches.
    let boot = || CORE.get().expect("booted").boot_info(&config());
    core.run_on(move || native::entry::run(0.0, boot, |_, _| 0.0))
        .expect("the core runs");
    // A frame's grant is queued by the runtime's thread before its turn is
    // kicked, so that thread is started.
    let (base, bytes) = core.mem();
    // SAFETY: the core's memory, reserved for the process; this core lays no
    // front, and receives into a page of its own.
    unsafe { native::attach(base.as_ptr() as usize, bytes, false) };
    settle();
    assert_eq!(turns(), 0, "paused from boot, nothing runs");

    native::wake();
    native::vsync(0.0);
    settle();
    assert_eq!(turns(), 0, "a frame begins nothing while paused");

    assert_eq!(native::run(0.0), Some(0.0));
    settle();
    assert_eq!(
        turns(),
        1,
        "one run, one turn, though it asked for the next"
    );

    native::resume();
    settle();
    assert_eq!(
        turns(),
        1,
        "resumed, a turn that asked for the next at once waits for a frame"
    );

    native::vsync(0.0);
    settle();
    assert_eq!(turns(), 2, "the frame runs it, and nothing after it");

    WAKE.store(1000.0_f64.to_bits(), Ordering::SeqCst);
    native::vsync(0.0);
    settle();
    assert_eq!(
        turns(),
        3,
        "the turn it asked for at once, at the next frame"
    );
    native::vsync(0.0);
    settle();
    assert_eq!(
        turns(),
        3,
        "a frame before a timed wake's time begins nothing"
    );

    WAKE.store(1.0_f64.to_bits(), Ordering::SeqCst);
    native::wake();
    native::vsync(0.0);
    settle();
    assert_eq!(turns(), 4, "a wake runs at the next frame, before the time");
    native::vsync(0.0);
    settle();
    assert_eq!(
        turns(),
        5,
        "the first frame past a timed wake's time runs it"
    );

    native::pause();
    settle();
    let paused_at = turns();
    settle();
    assert!(
        turns() <= paused_at + 1,
        "paused again, at most the turn under way ends"
    );
}
