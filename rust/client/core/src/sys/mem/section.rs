// pxl-core/sys/mem/section.rs

//! The core memory's sections, and where each begins:
//!
//!   front     the machine's (xpute-core machine.rs): the stack, the header,
//!             the rings and the GPU stream, then the linker's data; on a
//!             device the stack's and the data's ranges stand empty
//!   .fixed    ranges placed once at boot, sized at compile time, never
//!             freed (xpute-core mem/range.rs)
//!   heap      everything left, one allocator
//!
//! Only the core's own formats are in `.fixed`; an external one (glTF, PNG)
//! has no bound to size it by and is read into the heap (sys/io/loader.rs).
//! Which ranges there are, and how large, is the roof's (door/layout.rs),
//! which hands each owner its own at boot. Everything ahead of the heap is
//! held to the machine's `min_heap_bytes`, at compile time (`Machine::check`)
//! and again at boot.

use xpute_core::global::Global;
use xpute_core::mem::section::Section;

use crate::machine::MACHINE;

static FIXED: Global<Option<Section>> = Global::new();

/// The memory past the front laid out: `.fixed`'s section, the heap after
/// it held to the machine's least. `fixed_bytes` is the roof's
/// (door/layout.rs `FIXED_BYTES`).
pub fn lay_out(bytes: u32, data_end: u32, fixed_bytes: u32) -> Section {
    let region = MACHINE.region_bytes;
    let end = bytes / region * region;
    let heap = MACHINE.heap_at(data_end, fixed_bytes);
    xpute_kit::ensure!(heap + MACHINE.min_heap_bytes <= end, ENOMEM, heap, bytes);
    Section {
        off: MACHINE.fixed_at(data_end),
        bytes: fixed_bytes,
    }
}

pub fn set_fixed(fixed: Section) {
    FIXED.set(Some(fixed));
}

/// None before boot lays the memory out.
pub fn fixed() -> Option<Section> {
    *FIXED.get(|| None)
}
