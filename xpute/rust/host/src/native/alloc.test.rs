// xpute-host/native/alloc.test.rs

// A binary of its own, whose global allocator is the guest's.
#![cfg(unix)]

use std::sync::OnceLock;

use xpute_guest::mem::section::PAGE_BYTES;
use xpute_host::native::{self, Alloc, Config, Guest};

#[global_allocator]
static ALLOC: Alloc = Alloc;

const MEM_BYTES: u32 = 8 << 20;
const HEAP_AT: u32 = 16 * PAGE_BYTES;

fn guest() -> Guest {
    static GUEST: OnceLock<Guest> = OnceLock::new();
    *GUEST.get_or_init(|| {
        native::boot(&Config {
            mem_bytes: MEM_BYTES,
            data_end: 0,
            heap_at: HEAP_AT,
            stack_bytes: 1 << 20,
            thread_name: "test-guest",
            clock: native::now,
        })
    })
}

/// Whether `p` lies in the guest's heap.
fn in_heap(p: *const u8) -> bool {
    let (at, bytes) = guest().mem();
    let at = at.as_ptr().addr();
    (at + HEAP_AT as usize..at + bytes).contains(&p.addr())
}

#[test]
fn block_ownership() {
    let g = guest();
    let theirs = g.run(|| Box::into_raw(Box::new([7u8; 100])).expose_provenance());
    assert!(in_heap(core::ptr::with_exposed_provenance(theirs)));
    let ours = Box::new([7u8; 100]);
    assert!(!in_heap(ours.as_ptr()));
    // SAFETY: the box made on the guest's thread, given back here.
    drop(unsafe { Box::from_raw(core::ptr::with_exposed_provenance_mut::<[u8; 100]>(theirs)) });
}

#[test]
fn free_by_addr() {
    let g = guest();
    // The heap's block, let go here, goes back to the heap.
    let made = g.run(|| vec![1u32; 1000]);
    assert!(in_heap(made.as_ptr().cast()));
    drop(made);
    // The system's block, let go on the guest's thread, goes back to the system.
    let mine = vec![2u32; 1000];
    g.run(move || drop(mine));
}

#[test]
fn grow_across_moves() {
    let g = guest();
    let mut v = vec![3u8; 16];
    assert!(!in_heap(v.as_ptr()));
    let v = g.run(move || {
        v.reserve(1 << 16);
        v
    });
    assert!(in_heap(v.as_ptr()));
    assert_eq!(&v[..16], &[3u8; 16]);
    let mut v = v;
    v.reserve(1 << 18);
    assert!(!in_heap(v.as_ptr()));
    assert_eq!(&v[..16], &[3u8; 16]);
}

#[test]
fn heap_ends_with_mem() {
    let g = guest();
    let (at, bytes) = g.mem();
    let end = at.as_ptr().addr() + bytes;
    let block = g.run(|| vec![0u8; 1 << 20]);
    assert!(block.as_ptr().addr() + block.len() <= end);
    assert!(g.heap_stat().range <= bytes);
}
