// xpute-runtime/native/alloc.test.rs

// A binary of its own, whose global allocator is the core's.
#![cfg(unix)]

use std::sync::OnceLock;

use xpute_core::mem::section::PAGE_BYTES;
use xpute_runtime::native::{self, Alloc, Config, Core};

#[global_allocator]
static ALLOC: Alloc = Alloc;

const MEM_BYTES: u32 = 8 << 20;
const HEAP_AT: u32 = 16 * PAGE_BYTES;

fn core() -> Core {
    static CORE: OnceLock<Core> = OnceLock::new();
    *CORE.get_or_init(|| {
        native::boot(
            &Config {
                mem_bytes: MEM_BYTES,
                data_end: 0,
                heap_at: HEAP_AT,
                stack_bytes: 1 << 20,
                thread_name: "test-core",
                now: native::now,
                stop_hook: None,
                admit_frame: None,
                publish: None,
                ready: || true,
            },
            |_| -1.0,
        )
    })
}

/// Whether `p` lies in the core's heap.
fn in_heap(p: *const u8) -> bool {
    let (at, bytes) = core().mem();
    let at = at.as_ptr().addr();
    (at + HEAP_AT as usize..at + bytes).contains(&p.addr())
}

#[test]
fn block_ownership() {
    let g = core();
    let theirs = g
        .run_on(|| Box::into_raw(Box::new([7u8; 100])).expose_provenance())
        .expect("the core has stopped");
    assert!(in_heap(core::ptr::with_exposed_provenance(theirs)));
    let ours = Box::new([7u8; 100]);
    assert!(!in_heap(ours.as_ptr()));
    // SAFETY: the box made on the core's thread, given back here.
    drop(unsafe { Box::from_raw(core::ptr::with_exposed_provenance_mut::<[u8; 100]>(theirs)) });
}

#[test]
fn free_by_addr() {
    let g = core();
    // The heap's block, let go here, goes back to the heap.
    let made = g.run_on(|| vec![1u32; 1000]).expect("the core has stopped");
    assert!(in_heap(made.as_ptr().cast()));
    drop(made);
    // The system's block, let go on the core's thread, goes back to the system.
    let mine = vec![2u32; 1000];
    g.run_on(move || drop(mine)).expect("the core has stopped");
}

#[test]
fn grow_across_moves() {
    let g = core();
    let mut v = vec![3u8; 16];
    assert!(!in_heap(v.as_ptr()));
    let v = g
        .run_on(move || {
            v.reserve(1 << 16);
            v
        })
        .expect("the core has stopped");
    assert!(in_heap(v.as_ptr()));
    assert_eq!(&v[..16], &[3u8; 16]);
    let mut v = v;
    v.reserve(1 << 18);
    assert!(!in_heap(v.as_ptr()));
    assert_eq!(&v[..16], &[3u8; 16]);
}

#[test]
fn heap_ends_with_mem() {
    let g = core();
    let (at, bytes) = g.mem();
    let end = at.as_ptr().addr() + bytes;
    let block = g
        .run_on(|| vec![0u8; 1 << 20])
        .expect("the core has stopped");
    assert!(block.as_ptr().addr() + block.len() <= end);
    assert!(g.heap_stat().range <= bytes);
}
