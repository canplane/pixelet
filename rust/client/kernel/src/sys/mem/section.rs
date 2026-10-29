// pxl-kernel/sys/mem/section.rs

//! Which sections the memory has and where each begins:
//!
//!   module    stack first (an overflow traps), the rings, then the linker's data;
//!             on a device the stack's and the data's ranges stand empty
//!   .fixed    ranges sized at compile time, never freed (fixed.rs)
//!   heap      everything left, one allocator
//!
//! `.fixed` is not `.bss` on purpose: its size is a decision about how much
//! world is resident, not compiler output. Everything ahead of the heap is
//! checked against MIN_HEAP_BYTES, at compile time and again at boot.

use xpute_guest::mem::section::{align_up, Section};

use xpute_guest::global::Global;

// Shared with the shell and the linker through mem.json.
pub use crate::sys::mem::spec::{
    DESKTOP_BYTES, GLOBAL_BASE, MIN_HEAP_BYTES, PHONE_BYTES, REGION_BYTES,
};
use xpute_guest::mem::section::PAGE_BYTES;

pub const fn page_up(n: u32) -> u32 {
    align_up(n, PAGE_BYTES)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub heap: Section,
}

pub fn fixed_at(data_end: u32) -> u32 {
    page_up(data_end)
}

/// `fixed_bytes` is the roof's (door/layout.rs `FIXED_BYTES`).
pub fn lay_out(bytes: u32, data_end: u32, fixed_bytes: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    let heap = fixed_at(data_end) + fixed_bytes;
    xpute_kit::ensure!(
        heap + MIN_HEAP_BYTES <= regions * REGION_BYTES,
        ENOMEM,
        heap,
        bytes
    );
    Map {
        fixed: Section {
            off: fixed_at(data_end),
            bytes: fixed_bytes,
        },
        heap: Section {
            off: heap,
            bytes: regions * REGION_BYTES - heap,
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
