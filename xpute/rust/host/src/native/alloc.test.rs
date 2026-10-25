// xpute-host/native/alloc.test.rs

// A binary of its own, whose global allocator is the guest's.
#![cfg(unix)]

use std::sync::OnceLock;

use xpute_host::native::{self, Alloc, Guest, Plan};

#[global_allocator]
static ALLOC: Alloc = Alloc;

const MEMORY_BYTES: u32 = 8 << 20;

fn guest() -> Guest {
    static GUEST: OnceLock<Guest> = OnceLock::new();
    *GUEST.get_or_init(|| native::boot(&Plan::new(MEMORY_BYTES, 1 << 16)))
}

/// Whether `p` lies in the guest's heap.
fn in_heap(p: *const u8) -> bool {
    let (at, bytes) = guest().memory();
    let at = at.as_ptr().addr();
    (at + (1 << 16)..at + bytes).contains(&p.addr())
}

#[test]
fn the_guests_blocks_are_its_memorys_and_others_the_systems() {
    let g = guest();
    let theirs = g.run(|| Box::into_raw(Box::new([7u8; 100])).expose_provenance());
    assert!(in_heap(core::ptr::with_exposed_provenance(theirs)));
    let ours = Box::new([7u8; 100]);
    assert!(!in_heap(ours.as_ptr()));
    // SAFETY: the box made on the guest's thread, given back here.
    drop(unsafe { Box::from_raw(core::ptr::with_exposed_provenance_mut::<[u8; 100]>(theirs)) });
}

#[test]
fn a_block_goes_back_to_whichever_holds_its_address() {
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
fn a_block_that_grows_across_the_line_moves() {
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
fn the_heap_ends_with_the_memory() {
    let g = guest();
    let (at, bytes) = g.memory();
    let end = at.as_ptr().addr() + bytes;
    let block = g.run(|| vec![0u8; 1 << 20]);
    assert!(block.as_ptr().addr() + block.len() <= end);
    assert!(g.heap_stat().range <= bytes);
}
