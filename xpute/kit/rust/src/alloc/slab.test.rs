// xpute-kit/alloc/slab.test.rs

use super::*;
use crate::alloc::buddy_tree::Range;
use std::alloc::Layout;
use std::sync::OnceLock;

const MIN: usize = 12;
const MAX: usize = 20;
const SPLIT_SZ: usize = (1 << (MAX - MIN)) / 8;
const PAGE: usize = 1 << MIN;

/// `arena` reserves a test's range once and never frees it. `Range`'s
/// functions take no receiver, so the base lives in a static, and the
/// allocator over the range outlives any one test.
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
            type Split = [u8; SPLIT_SZ];
            const SPLIT_INIT: Self::Split = [0; SPLIT_SZ];
            fn base() -> usize {
                arena(&$cell)
            }
            fn end() -> usize {
                arena(&$cell) + (1 << MAX)
            }
        }
    };
}

range!(SlotRange, SLOTS_AT);
range!(EmptyRun, EMPTY_RUN_AT);
range!(Whole, WHOLE_AT);
range!(RunRange, RUNS_AT);

fn small() -> Layout {
    Layout::from_size_align(24, 8).unwrap()
}

#[test]
fn slot_contents_survive() {
    let heap: SlabMalloc<SlotRange> = SlabMalloc::new();
    let sizes = [1usize, 16, 17, 24, 64, 200, 1000, 4096, 9000];
    let mut live: Vec<(*mut u8, usize, u8)> = Vec::new();

    for (i, sz) in sizes.iter().cycle().take(90).enumerate() {
        let layout = Layout::from_size_align(*sz, 8).unwrap();
        // SAFETY: the range is the test's own.
        let p = unsafe { heap.alloc(layout) };
        if p.is_null() {
            continue;
        }
        // SAFETY: `sz` bytes the allocator just handed out.
        unsafe { ptr::write_bytes(p, i as u8, *sz) };
        live.push((p, *sz, i as u8));
    }
    assert!(
        live.len() > 40,
        "the range served only {} of 90 requests",
        live.len()
    );

    for (p, sz, _) in live.iter().step_by(2) {
        // SAFETY: from this allocator, not yet given back.
        unsafe { heap.dealloc(*p, Layout::from_size_align(*sz, 8).unwrap()) };
    }
    live = live.into_iter().skip(1).step_by(2).collect();

    for sz in sizes {
        let layout = Layout::from_size_align(sz, 8).unwrap();
        // SAFETY: as above.
        let p = unsafe { heap.alloc(layout) };
        if !p.is_null() {
            // SAFETY: `sz` bytes the allocator just handed out.
            unsafe { ptr::write_bytes(p, 0xff, sz) };
        }
    }

    for (p, sz, byte) in &live {
        // SAFETY: still live, and `sz` bytes long.
        let bytes = unsafe { core::slice::from_raw_parts(*p, *sz) };
        assert!(
            bytes.iter().all(|b| b == byte),
            "a slot written with {byte:#04x} was written over"
        );
    }
}

#[test]
fn empty_run_returned() {
    let heap: SlabMalloc<EmptyRun> = SlabMalloc::new();
    // SAFETY: the range is the test's own.
    let first = unsafe { heap.alloc(small()) };
    assert!(!first.is_null());
    let one_run = heap.pages().live();
    assert!(one_run >= PAGE, "a slot took no page from the level below");

    let mut live = vec![first];
    while heap.pages().live() == one_run {
        // SAFETY: the range is the test's own.
        let p = unsafe { heap.alloc(small()) };
        assert!(!p.is_null(), "the range ran out before one run was full");
        live.push(p);
    }
    // The last slot is the second run's only one. Freeing it first gives the
    // second run back and leaves the first run's slots.
    let spilled = live.pop().unwrap();
    // SAFETY: `spilled` was served at `small()`, and leaves `live` as it is
    // freed.
    unsafe { heap.dealloc(spilled, small()) };
    assert_eq!(
        heap.pages().live(),
        one_run,
        "the second run did not go back"
    );

    for p in live {
        // SAFETY: each of `live` was served at `small()` and is freed once.
        unsafe { heap.dealloc(p, small()) };
    }
    assert_eq!(
        heap.pages().live(),
        0,
        "the last slot of a run did not give the run back"
    );
}

#[test]
fn page_requests_delegated() {
    let heap: SlabMalloc<Whole> = SlabMalloc::new();
    for sz in [PAGE, PAGE + 1, PAGE * 4] {
        let layout = Layout::from_size_align(sz, 8).unwrap();
        let before = heap.pages().live();
        // SAFETY: the range is the test's own.
        let p = unsafe { heap.alloc(layout) };
        assert!(!p.is_null());
        let took = heap.pages().live() - before;
        assert!(
            took >= sz,
            "a request of {sz} took only {took} from the pages"
        );
        assert_eq!(
            p as usize % PAGE,
            0,
            "a whole-page request was not page-aligned"
        );
        // SAFETY: `p` was served at `layout` just above.
        unsafe { heap.dealloc(p, layout) };
        assert_eq!(heap.pages().live(), before, "it did not go back whole");
    }
}

#[test]
fn slots_inside_runs() {
    let heap: SlabMalloc<RunRange> = SlabMalloc::new();
    let sizes = [16usize, 24, 64, 300, 1000, PAGE, PAGE * 3];
    let mut live: Vec<(*mut u8, Layout)> = Vec::new();

    let check = |heap: &SlabMalloc<RunRange>| {
        assert!(
            heap.slot_live() <= heap.run_live(),
            "slots out past the runs they are carved from"
        );
        assert!(
            heap.run_live() <= heap.pages().live(),
            "runs borrowed past what the pages handed out"
        );
        assert!(
            heap.pages().live() <= heap.range_sz(),
            "the pages handed out past the range"
        );
    };

    for round in 0..4 {
        for (k, sz) in sizes.iter().enumerate() {
            let layout = Layout::from_size_align(*sz, 8).unwrap();
            // SAFETY: the range is the test's own.
            let p = unsafe { heap.alloc(layout) };
            assert!(!p.is_null(), "the range ran out at {sz} on round {round}");
            live.push((p, layout));
            check(&heap);
            // One allocation is freed for every two made, so runs are left
            // partly full.
            if k % 2 == 1 {
                let (q, l) = live.remove(0);
                // SAFETY: `q` was served at `l`, and leaves `live` as it is
                // freed.
                unsafe { heap.dealloc(q, l) };
                check(&heap);
            }
        }
    }

    for (p, l) in live.drain(..) {
        // SAFETY: `p` was served at `l`, and leaves `live` as it is freed.
        unsafe { heap.dealloc(p, l) };
        check(&heap);
    }
    assert_eq!(
        heap.slot_live(),
        0,
        "a slot was left counted after its free"
    );
    assert_eq!(
        heap.run_live(),
        0,
        "a run was left counted after it went back"
    );
    assert_eq!(
        heap.pages().live(),
        0,
        "the pages were left holding something"
    );
}

#[test]
fn whole_page_not_slab() {
    let heap: SlabMalloc<Whole> = SlabMalloc::new();
    let layout = Layout::from_size_align(PAGE * 2, 8).unwrap();
    let (run, slot) = (heap.run_live(), heap.slot_live());
    // SAFETY: the range is the test's own.
    let p = unsafe { heap.alloc(layout) };
    assert!(!p.is_null());
    assert_eq!(
        (heap.run_live(), heap.slot_live()),
        (run, slot),
        "a whole-page request was counted as carving"
    );
    assert!(
        heap.pages().live() >= run + PAGE * 2,
        "it did not come from the pages"
    );
    // SAFETY: the block this test was just given, at its own layout.
    unsafe { heap.dealloc(p, layout) };
}
