// pxl-kernel/mem/section.rs

//! Which sections the memory has and where each begins:
//!
//!   module    stack first (an overflow traps), the rings, then the linker's data
//!   .fixed    ranges sized at compile time, never freed (fixed.rs)
//!   heap      everything left, one allocator
//!
//! `.fixed` is not `.bss` on purpose: its size is a decision about how much
//! world is resident, not compiler output. Everything ahead of the heap is
//! checked against MIN_HEAP_BYTES, at compile time and again at boot.

use xpute_runtime::mem::section::{align_up, Section};

use xpute_runtime::global::Global;

// Shared with the shell and the linker through mem.json.
pub use crate::mem::spec::{DESKTOP_BYTES, GLOBAL_BASE, MIN_HEAP_BYTES, PAGE_BYTES, PHONE_BYTES, REGION_BYTES};

pub const fn page_up(n: u32) -> u32 {
    align_up(n, PAGE_BYTES)
}

pub use crate::mem::fixed::BYTES as FIXED_BYTES;

const _: () = assert!(
    REGION_BYTES.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES) && FIXED_BYTES.is_multiple_of(PAGE_BYTES),
    "a section begins on a page, so every block in it begins on one"
);

/// The lowest the heap could begin: the linker's data cannot end before `--global-base`.
const _: () = assert!(
    GLOBAL_BASE + FIXED_BYTES + MIN_HEAP_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the module's front, the fixed ranges and the heap they are budgeted against"
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub heap: Section,
}

pub fn fixed_at(data_end: u32) -> u32 {
    page_up(data_end)
}

/// A pure function of the linker's answer: the global allocator asks for
/// its first block before anything has laid the memory out.
pub fn heap_at(data_end: u32) -> u32 {
    fixed_at(data_end) + FIXED_BYTES
}

pub fn lay_out(bytes: u32, data_end: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    let heap = heap_at(data_end);
    xpute_core::ensure!(heap + MIN_HEAP_BYTES <= regions * REGION_BYTES, ENOMEM, heap, bytes);
    Map {
        fixed: Section {
            offset: fixed_at(data_end),
            bytes: FIXED_BYTES,
            align: PAGE_BYTES,
        },
        heap: Section {
            offset: heap,
            bytes: regions * REGION_BYTES - heap,
            align: PAGE_BYTES,
        },
    }
}

static MAP: Global<Option<Map>> = Global::new();

pub fn set_map(map: Map) {
    MAP.set(Some(map));
}

/// None under a native test, which has no sections.
pub fn map() -> Option<Map> {
    *MAP.get(|| None)
}
