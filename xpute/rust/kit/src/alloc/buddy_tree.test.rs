// xpute-kit/alloc/buddy_tree.test.rs

use super::*;
use std::alloc::Layout;
use std::sync::OnceLock;

const MIN: usize = 12;
const MAX: usize = 20;
const SPLIT_BYTES: usize = (1 << (MAX - MIN)) / 8;

/// Made once and never freed: `Range` has no receiver, and the allocator
/// over it outlives any one test.
fn arena(cell: &'static OnceLock<usize>) -> usize {
    *cell.get_or_init(|| {
        let layout = Layout::from_size_align(1 << MAX, 1 << MAX).unwrap();
        // SAFETY: a layout of non-zero size.
        let at = unsafe { std::alloc::alloc(layout) };
        assert!(!at.is_null(), "the test could not reserve its own range");
        at as usize
    })
}

macro_rules! range {
    ($name:ident, $cell:ident) => {
        static $cell: OnceLock<usize> = OnceLock::new();
        struct $name;
        impl Range for $name {
            const MIN_LOG2: usize = MIN;
            const MAX_LOG2: usize = MAX;
            type Split = [u8; SPLIT_BYTES];
            const SPLIT_INIT: Self::Split = [0; SPLIT_BYTES];
            fn base() -> usize {
                arena(&$cell)
            }
            fn end() -> usize {
                arena(&$cell) + (1 << MAX)
            }
        }
    };
}

range!(Spread, SPREAD_AT);
range!(Buddies, BUDDIES_AT);
range!(Full, FULL_AT);
range!(Reused, REUSED_AT);

#[test]
fn what_was_written_in_a_block_is_still_there_after_its_neighbors_come_and_go() {
    let heap: BuddyMalloc<Spread> = BuddyMalloc::new();
    let (base, end) = (Spread::base(), Spread::end());
    let sizes = [1usize, 4096, 4097, 8192, 100, 16384, 40, 65536];
    let mut live: Vec<(*mut u8, usize, u8)> = Vec::new();

    for (i, size) in sizes.iter().cycle().take(24).enumerate() {
        // SAFETY: the range is the test's own.
        let p = unsafe { heap.malloc(*size) };
        if p.is_null() {
            continue;
        }
        let at = p as usize;
        assert!(at >= base && at + size <= end, "a block of {size} at {at:#x} is outside [{base:#x}, {end:#x})");
        // SAFETY: `size` bytes the allocator just handed out.
        unsafe { core::ptr::write_bytes(p, i as u8, *size) };
        live.push((p, *size, i as u8));
    }
    assert!(live.len() > 8, "the range served only {} of 24 requests", live.len());

    for (p, size, _) in live.iter().step_by(2) {
        // SAFETY: from this allocator, not yet given back.
        unsafe { heap.free(*p, *size) };
    }
    live = live.into_iter().skip(1).step_by(2).collect();

    for size in sizes {
        // SAFETY: as above.
        let p = unsafe { heap.malloc(size) };
        if !p.is_null() {
            // SAFETY: `size` bytes the allocator just handed out.
            unsafe { core::ptr::write_bytes(p, 0xff, size) };
        }
    }

    for (p, size, byte) in &live {
        // SAFETY: still live, and `size` bytes long.
        let bytes = unsafe { core::slice::from_raw_parts(*p, *size) };
        assert!(bytes.iter().all(|b| b == byte), "a block written with {byte:#04x} was written over");
    }
}

#[test]
fn two_buddies_freed_are_the_block_they_came_from() {
    let heap: BuddyMalloc<Buddies> = BuddyMalloc::new();
    let one = 1 << MIN;
    // SAFETY: the range is the test's own, here and below.
    let (a, b) = unsafe { (heap.malloc(one), heap.malloc(one)) };
    assert!(!a.is_null() && !b.is_null());
    let low = a.min(b);
    unsafe {
        heap.free(a, one);
        heap.free(b, one);
    }
    let both = unsafe { heap.malloc(one * 2) };
    assert_eq!(both, low, "the pair did not join: {both:?} is not the lower half {low:?}");
}

#[test]
fn a_request_past_the_range_is_refused_and_leaves_what_is_live_alone() {
    let heap: BuddyMalloc<Full> = BuddyMalloc::new();
    let one = 1 << MIN;
    // SAFETY: the range is the test's own, here and below.
    let first = unsafe { heap.malloc(one) };
    assert!(!first.is_null());
    unsafe { core::ptr::write_bytes(first, 0x5a, one) };

    let mut served = 1;
    while !unsafe { heap.malloc(one) }.is_null() {
        served += 1;
        assert!(served <= (1 << (MAX - MIN)) + 1, "the range served more blocks than it holds");
    }
    assert!(heap.live() <= heap.range_bytes(), "more is live than the range has");

    // SAFETY: still live.
    let bytes = unsafe { core::slice::from_raw_parts(first, one) };
    assert!(bytes.iter().all(|b| *b == 0x5a), "a refusal wrote over a live block");
}

#[test]
fn what_is_freed_is_handed_out_again_rather_than_more_of_the_range() {
    let heap: BuddyMalloc<Reused> = BuddyMalloc::new();
    let size = 1 << (MIN + 2);
    // SAFETY: the range is the test's own, here and below.
    let p = unsafe { heap.malloc(size) };
    assert!(!p.is_null());
    let reached = heap.used();

    for _ in 0..8 {
        unsafe { heap.free(p, size) };
        let q = unsafe { heap.malloc(size) };
        assert_eq!(q, p, "the same request after the same free moved");
        assert_eq!(heap.used(), reached, "a reuse reached further into the range");
    }
    assert!(heap.releases() >= 8);
}
