// xpute-host/native.test.rs

// A binary of its own: a process boots one guest and sets one base.
#![cfg(unix)]

use std::sync::OnceLock;

use xpute_guest::global::{off_marked_thread, on_marked_thread};
use xpute_guest::mem::section::PAGE_BYTES;
use xpute_host::native::{self, Guest, Plan};

const MEMORY_BYTES: u32 = 4 << 20;
const HEAP_AT: u32 = 16 * PAGE_BYTES;

fn plan(heap_at: u32) -> Plan {
    Plan {
        memory_bytes: MEMORY_BYTES,
        data_end: 0,
        heap_at,
        stack_bytes: 1 << 20,
        thread_name: "test-guest",
        clock: || 42.0,
    }
}

fn guest() -> Guest {
    static GUEST: OnceLock<Guest> = OnceLock::new();
    *GUEST.get_or_init(|| native::boot(&plan(HEAP_AT)))
}

#[test]
fn the_memory_is_aligned_zero_and_the_base() {
    let (at, bytes) = guest().memory();
    assert_eq!(bytes, MEMORY_BYTES as usize);
    assert!(at.as_ptr().addr().is_multiple_of(PAGE_BYTES as usize));
    let base = guest().run(|| xpute_guest::mem::core().base());
    assert_eq!(base, at.as_ptr().addr());
    // SAFETY: the reserved memory, mapped for the process; nothing else reads
    // its last page here.
    let last = unsafe { core::slice::from_raw_parts(at.as_ptr().add(bytes - 4096), 4096) };
    assert!(last.iter().all(|&b| b == 0));
}

#[test]
fn a_call_runs_on_the_guests_named_thread() {
    let (name, marked) = guest().run(|| {
        (
            std::thread::current().name().map(str::to_owned),
            on_marked_thread(),
        )
    });
    assert_eq!(name.as_deref(), Some("test-guest"));
    assert!(marked);
    assert!(off_marked_thread(), "a caller is not the guest's thread");
}

#[test]
fn callers_on_several_threads_take_turns() {
    let g = guest();
    let sums: Vec<u64> = std::thread::scope(|s| {
        let calls: Vec<_> = (0..4u64)
            .map(|k| s.spawn(move || (0..100).map(|n| g.run(move || k * n)).sum::<u64>()))
            .collect();
        calls.into_iter().map(|c| c.join().unwrap()).collect()
    });
    assert_eq!(sums, [0, 4950, 9900, 14850]);
}

#[test]
fn while_idle_runs_here() {
    let here = std::thread::current().id();
    assert_eq!(guest().while_idle(|| std::thread::current().id()), here);
}

#[test]
fn a_second_boot_is_refused() {
    guest();
    let again = std::panic::catch_unwind(|| native::boot(&plan(HEAP_AT)));
    assert!(again.is_err());
}

#[test]
fn a_heap_off_a_page_or_past_the_memory_is_refused_before_the_boot() {
    // The slab aligns a block by its offset from the heap's first byte.
    assert!(std::panic::catch_unwind(|| native::boot(&plan(100))).is_err());
    assert!(std::panic::catch_unwind(|| native::boot(&plan(MEMORY_BYTES))).is_err());
    // Neither took the process's one boot.
    let (_, bytes) = guest().memory();
    assert_eq!(bytes, MEMORY_BYTES as usize);
}

#[test]
fn the_first_intr_installs_the_plans_clock() {
    let boot = || guest().boot_info(&plan(HEAP_AT));
    assert_eq!(
        guest().run(move || native::intr(0.0, boot, |_, _| xpute_guest::clock::now())),
        42.0
    );
}

#[test]
fn while_idle_from_the_guests_own_thread_or_inside_a_call_is_refused() {
    let g = guest();
    let inner = g.run(move || std::panic::catch_unwind(move || g.while_idle(|| ())).is_err());
    assert!(inner, "the guest's thread would wait for its caller");
    let nested =
        g.while_idle(move || std::panic::catch_unwind(move || g.while_idle(|| ())).is_err());
    assert!(
        nested,
        "a second hold on one thread would wait for the first"
    );
    let called = g.while_idle(move || std::panic::catch_unwind(move || g.run(|| ())).is_err());
    assert!(called, "a call inside while_idle would wait for it");
    // Each refusal left the handoff as it was.
    assert_eq!(g.run(|| 7), 7);
    assert_eq!(g.while_idle(|| 8), 8);
}

#[test]
fn a_call_from_the_guests_own_thread_is_refused() {
    let g = guest();
    let inner = g.run(move || std::panic::catch_unwind(move || g.run(|| ())).is_err());
    assert!(inner, "it would wait for itself");
}

#[test]
fn the_base_is_the_guests_thread_to_set() {
    guest();
    assert!(std::panic::catch_unwind(|| unsafe { xpute_guest::mem::set(0, 0) }).is_err());
}

#[test]
fn the_clock_runs_forward() {
    let a = native::now();
    let b = native::now();
    assert!(b >= a);
}
