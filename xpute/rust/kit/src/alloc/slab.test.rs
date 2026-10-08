// xpute-kit/alloc/slab.test.rs

use super::*;
use crate::alloc::buddy_tree::Range;
use std::alloc::Layout;
use std::sync::OnceLock;

const MIN: usize = 12;
const MAX: usize = 20;
const SPLIT_BYTES: usize = (1 << (MAX - MIN)) / 8;
const PAGE: usize = 1 << MIN;

/// Made once and never freed: `Range` has no receiver.
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

range!(Slots, SLOTS_AT);
range!(Returned, RETURNED_AT);
range!(Whole, WHOLE_AT);
range!(Held, HELD_AT);

fn small() -> Layout {
    Layout::from_size_align(24, 8).unwrap()
}

#[test]
fn what_was_written_in_a_slot_is_still_there_after_its_neighbors_come_and_go() {
    let heap: SlabMalloc<Slots> = SlabMalloc::new();
    let sizes = [1usize, 16, 17, 24, 64, 200, 1000, 4096, 9000];
    let mut live: Vec<(*mut u8, usize, u8)> = Vec::new();

    for (i, size) in sizes.iter().cycle().take(90).enumerate() {
        let layout = Layout::from_size_align(*size, 8).unwrap();
        // SAFETY: the range is the test's own.
        let p = unsafe { heap.alloc(layout) };
        if p.is_null() {
            continue;
        }
        // SAFETY: `size` bytes the allocator just handed out.
        unsafe { core::ptr::write_bytes(p, i as u8, *size) };
        live.push((p, *size, i as u8));
    }
    assert!(live.len() > 40, "the range served only {} of 90 requests", live.len());

    for (p, size, _) in live.iter().step_by(2) {
        // SAFETY: from this allocator, not yet given back.
        unsafe { heap.dealloc(*p, Layout::from_size_align(*size, 8).unwrap()) };
    }
    live = live.into_iter().skip(1).step_by(2).collect();

    for size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        // SAFETY: as above.
        let p = unsafe { heap.alloc(layout) };
        if !p.is_null() {
            // SAFETY: `size` bytes the allocator just handed out.
            unsafe { core::ptr::write_bytes(p, 0xff, size) };
        }
    }

    for (p, size, byte) in &live {
        // SAFETY: still live, and `size` bytes long.
        let bytes = unsafe { core::slice::from_raw_parts(*p, *size) };
        assert!(bytes.iter().all(|b| b == byte), "a slot written with {byte:#04x} was written over");
    }
}

#[test]
fn a_run_goes_back_to_the_pages_when_its_last_slot_does() {
    let heap: SlabMalloc<Returned> = SlabMalloc::new();
    // SAFETY: the range is the test's own, here and below.
    let first = unsafe { heap.alloc(small()) };
    assert!(!first.is_null());
    let one_run = heap.pages().live();
    assert!(one_run >= PAGE, "a slot took no page from the level below");

    let mut live = vec![first];
    while heap.pages().live() == one_run {
        let p = unsafe { heap.alloc(small()) };
        assert!(!p.is_null(), "the range ran out before one run was full");
        live.push(p);
    }
    // Free the second run's slot first, leaving exactly the first run's.
    let spilled = live.pop().unwrap();
    unsafe { heap.dealloc(spilled, small()) };
    assert_eq!(heap.pages().live(), one_run, "the second run did not go back");

    for p in live {
        unsafe { heap.dealloc(p, small()) };
    }
    assert_eq!(heap.pages().live(), 0, "the last slot of a run did not give the run back");
}

#[test]
fn a_request_of_a_page_or_more_is_the_page_allocator_s_own() {
    let heap: SlabMalloc<Whole> = SlabMalloc::new();
    for size in [PAGE, PAGE + 1, PAGE * 4] {
        let layout = Layout::from_size_align(size, 8).unwrap();
        let before = heap.pages().live();
        // SAFETY: the range is the test's own, here and below.
        let p = unsafe { heap.alloc(layout) };
        assert!(!p.is_null());
        let took = heap.pages().live() - before;
        assert!(took >= size, "a request of {size} took only {took} from the pages");
        assert_eq!(p as usize % PAGE, 0, "a whole-page request was not page-aligned");
        unsafe { heap.dealloc(p, layout) };
        assert_eq!(heap.pages().live(), before, "it did not go back whole");
    }
}

#[test]
fn what_is_out_in_slots_is_inside_the_runs_it_is_carved_from() {
    let heap: SlabMalloc<Held> = SlabMalloc::new();
    let sizes = [16usize, 24, 64, 300, 1000, PAGE, PAGE * 3];
    let mut live: Vec<(*mut u8, Layout)> = Vec::new();

    let check = |heap: &SlabMalloc<Held>| {
        assert!(heap.slot_live() <= heap.run_live(), "slots out past the runs they are carved from");
        assert!(heap.run_live() <= heap.pages().live(), "runs borrowed past what the pages handed out");
        assert!(heap.pages().live() <= heap.range_bytes(), "the pages handed out past the range");
    };

    for round in 0..4 {
        for (k, size) in sizes.iter().enumerate() {
            let layout = Layout::from_size_align(*size, 8).unwrap();
            // SAFETY: the range is the test's own, here and below.
            let p = unsafe { heap.alloc(layout) };
            assert!(!p.is_null(), "the range ran out at {size} on round {round}");
            live.push((p, layout));
            check(&heap);
            // Every other one, so runs are left partly full.
            if k % 2 == 1 {
                let (q, l) = live.remove(0);
                unsafe { heap.dealloc(q, l) };
                check(&heap);
            }
        }
    }

    for (p, l) in live.drain(..) {
        unsafe { heap.dealloc(p, l) };
        check(&heap);
    }
    assert_eq!(heap.slot_live(), 0, "a slot was left counted after its free");
    assert_eq!(heap.run_live(), 0, "a run was left counted after it went back");
    assert_eq!(heap.pages().live(), 0, "the pages were left holding something");
}

#[test]
fn a_whole_page_request_is_no_run_and_no_slot() {
    let heap: SlabMalloc<Whole> = SlabMalloc::new();
    let layout = Layout::from_size_align(PAGE * 2, 8).unwrap();
    let (run, slot) = (heap.run_live(), heap.slot_live());
    // SAFETY: the range is the test's own.
    let p = unsafe { heap.alloc(layout) };
    assert!(!p.is_null());
    assert_eq!((heap.run_live(), heap.slot_live()), (run, slot), "a whole-page request was counted as carving");
    assert!(heap.pages().live() >= run + PAGE * 2, "it did not come from the pages");
    // SAFETY: the block this test was just given, at its own layout.
    unsafe { heap.dealloc(p, layout) };
}
