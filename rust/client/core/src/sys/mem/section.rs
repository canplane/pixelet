// pxl-core/sys/mem/section.rs

//! The sections of the core memory, in the order they lie:
//!
//!   front     laid out by the machine (xpute-core machine.rs): the stack,
//!             the header, the rings and the GPU stream, then the linker's
//!             data. On a device the stack's and the data's ranges are empty.
//!   .fixed    ranges placed once at boot, sized at compile time and never
//!             freed (xpute-core mem/range.rs)
//!   heap      everything left, under one allocator
//!
//! Only the core's own formats live in `.fixed`. An external format (glTF,
//! PNG) has no bound to size a range by, so it is read into the heap
//! (sys/io/loader.rs). door/layout.rs decides which ranges `.fixed` holds
//! and how large each is, and hands each owner its range at boot. The front
//! and `.fixed` must leave the machine's `min_heap_bytes` to the heap;
//! `Machine::check` holds that at compile time, and `lay_out` again at boot.

use xpute_core::global::Global;
use xpute_core::mem::section::Section;

use crate::machine::MACHINE;

static FIXED: Global<Option<Section>> = Global::new();

/// `.fixed`'s section in a memory of `bytes`, placed after the data that
/// ends at `data_end`. The core stops with ENOMEM where the heap after it
/// would be smaller than the machine's `min_heap_bytes`. `fixed_bytes` is
/// door/layout.rs `FIXED_BYTES`.
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
