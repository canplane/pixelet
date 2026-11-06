// pxl-kernel/sys/mem/section.rs

//! Which sections the memory has and where each begins:
//!
//!   front     the machine's (xpute-core machine.rs): the stack, the header, the
//!             rings and the GPU stream, then the linker's data; on a device
//!             the stack's and the data's ranges stand empty
//!   .fixed    ranges sized at compile time, never freed (fixed.rs)
//!   heap      everything left, one allocator
//!
//! Everything ahead of the heap is checked against the machine's
//! `min_heap_bytes`, at compile time (`Machine::check`) and again at boot.

use xpute_core::mem::section::Section;

use crate::machine::MACHINE;
use xpute_core::global::Global;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub heap: Section,
}

/// `fixed_bytes` is the roof's (door/layout.rs `FIXED_BYTES`).
pub fn lay_out(bytes: u32, data_end: u32, fixed_bytes: u32) -> Map {
    let region = MACHINE.region_bytes;
    let end = bytes / region * region;
    let heap = MACHINE.heap_at(data_end, fixed_bytes);
    xpute_kit::ensure!(heap + MACHINE.min_heap_bytes <= end, ENOMEM, heap, bytes);
    Map {
        fixed: Section {
            off: MACHINE.fixed_at(data_end),
            bytes: fixed_bytes,
        },
        heap: Section {
            off: heap,
            bytes: end - heap,
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
