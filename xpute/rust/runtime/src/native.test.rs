// xpute-runtime/native.test.rs

// This test is a binary of its own, because a process boots one core and sets
// one memory base.
#![cfg(unix)]

use std::sync::OnceLock;

use xpute_core::global::{off_marked_thread, on_marked_thread};
use xpute_core::mem::section::PAGE_BYTES;
use xpute_runtime::native::{self, Config, Core};

const MEM_BYTES: u32 = 4 << 20;
const HEAP_AT: u32 = 16 * PAGE_BYTES;

fn cfg(heap_at: u32) -> Config {
    Config {
        mem_bytes: MEM_BYTES,
        heap_at,
        stack_bytes: 1 << 20,
        thread_name: "test-core",
        ready: || true,
        form: xpute_runtime::native::Form {
            data_end: 0,
            now: || 42.0,
            stop_hook: None,
            admit_frame: None,
            publish: None,
        },
    }
}

fn turn(_: f64) -> f64 {
    -1.0
}

fn core() -> Core {
    static CORE: OnceLock<Core> = OnceLock::new();
    *CORE.get_or_init(|| native::boot(&cfg(HEAP_AT), turn))
}

#[test]
fn mem_aligned_zeroed() {
    let (at, bytes) = core().mem();
    assert_eq!(bytes, MEM_BYTES as usize);
    assert!(at.as_ptr().addr().is_multiple_of(PAGE_BYTES as usize));
    let base = core()
        .run_on(|| xpute_core::mem::core().base())
        .expect("the core has stopped");
    assert_eq!(base, at.as_ptr().addr());
    // SAFETY: the reserved memory, mapped for the process; nothing else reads
    // its last page here.
    let last = unsafe { core::slice::from_raw_parts(at.as_ptr().add(bytes - 4096), 4096) };
    assert!(last.iter().all(|&b| b == 0));
}

#[test]
fn call_on_core_thread() {
    let (name, marked) = core()
        .run_on(|| {
            (
                std::thread::current().name().map(str::to_owned),
                on_marked_thread(),
            )
        })
        .expect("the core has stopped");
    assert_eq!(name.as_deref(), Some("test-core"));
    assert!(marked);
    assert!(off_marked_thread(), "a caller is not the core's thread");
}

#[test]
fn callers_take_turns() {
    let g = core();
    let sums: Vec<u64> = std::thread::scope(|s| {
        let calls: Vec<_> = (0..4u64)
            .map(|k| {
                s.spawn(move || {
                    (0..100)
                        .map(|n| g.run_on(move || k * n).expect("the core has stopped"))
                        .sum::<u64>()
                })
            })
            .collect();
        calls.into_iter().map(|c| c.join().unwrap()).collect()
    });
    assert_eq!(sums, [0, 4950, 9900, 14850]);
}

#[test]
fn call_answered_by_caller() {
    extern "C" fn answer(at: u32) -> i32 {
        assert!(off_marked_thread(), "an answer runs on the caller's thread");
        at as i32 + 1
    }
    native::answer_with(Some(answer));
    let g = core();
    let boot = g.boot_info(&cfg(HEAP_AT));
    assert_eq!(
        g.run_on(move || (boot.call)(41))
            .expect("the core has stopped"),
        42
    );
}

#[test]
fn while_idle_runs_here() {
    let here = std::thread::current().id();
    assert_eq!(core().while_idle(|| std::thread::current().id()), here);
}

#[test]
fn ring_waits_for_a_turn() {
    core();
    assert_eq!(native::ring(1.0), Some(-1.0));
}

#[test]
fn a_kick_runs_a_turn() {
    core();
    let _ = native::turns();
    native::kick_deadline(-1.0);
    assert_eq!(native::ring(0.0), Some(-1.0));
    let (_, turn_ms) = native::turns();
    assert!(turn_ms >= 0.0);
}

#[test]
fn second_boot_refused() {
    core();
    let again = std::panic::catch_unwind(|| native::boot(&cfg(HEAP_AT), turn));
    assert!(again.is_err());
}

#[test]
fn bad_heap_refused() {
    // A heap must begin on a page, because the slab aligns a block by its
    // offset from the heap's first byte.
    assert!(std::panic::catch_unwind(|| native::boot(&cfg(100), turn)).is_err());
    assert!(std::panic::catch_unwind(|| native::boot(&cfg(MEM_BYTES), turn)).is_err());
    // Neither refused boot used up the process's one boot.
    let (_, bytes) = core().mem();
    assert_eq!(bytes, MEM_BYTES as usize);
}

#[test]
fn first_intr_installs_clock() {
    let boot = || core().boot_info(&cfg(HEAP_AT));
    assert_eq!(
        core()
            .run_on(move || native::run(0.0, boot, |_, _| xpute_core::clock::now()))
            .expect("the core has stopped"),
        42.0
    );
}

#[test]
fn bad_while_idle_refused() {
    let g = core();
    let inner = g
        .run_on(move || std::panic::catch_unwind(move || g.while_idle(|| ())).is_err())
        .expect("the core has stopped");
    assert!(inner, "the core's thread would wait for its caller");
    let nested =
        g.while_idle(move || std::panic::catch_unwind(move || g.while_idle(|| ())).is_err());
    assert!(
        nested,
        "a second hold on one thread would wait for the first"
    );
    let called = g.while_idle(move || {
        std::panic::catch_unwind(move || g.run_on(|| ()).expect("the core has stopped")).is_err()
    });
    assert!(called, "a call inside while_idle would wait for it");
    // Each refusal left the handoff between the threads as it was.
    assert_eq!(g.run_on(|| 7).expect("the core has stopped"), 7);
    assert_eq!(g.while_idle(|| 8), 8);
}

#[test]
fn self_call_refused() {
    let g = core();
    let inner = g
        .run_on(move || {
            std::panic::catch_unwind(move || g.run_on(|| ()).expect("the core has stopped"))
                .is_err()
        })
        .expect("the core has stopped");
    assert!(inner, "it would wait for itself");
}

#[test]
fn base_set_by_core() {
    core();
    assert!(std::panic::catch_unwind(|| unsafe { xpute_core::mem::set(0, 0) }).is_err());
}

#[test]
fn the_clock_runs_forward() {
    let a = native::now();
    let b = native::now();
    assert!(b >= a);
}
