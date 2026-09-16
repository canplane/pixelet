// pxl_core/memory.rs

//! The kernel's policy over the memory the central scheduler gives it: which
//! sections there are and where each begins. Cutting the memory into sections
//! is the mechanism's (xpute_core::sys::memory); what a section holds, and
//! what "full" means in it, is decided here
//! (issue/20260913-sections-of-the-core-memory-and-what-full-means.md).
//!
//! Sections, in address order:
//!
//!   module   the stack at the bottom (`--stack-first`, so an overflow wraps
//!            past zero and traps), the rings at addresses pxl-wasm's
//!            build.rs fixes, and then the module's own data and bss, which
//!            the same build.rs keeps out of the rings' way with
//!            `--global-base`. Not the module's heap: that is past the slab,
//!            at HEAP_AT.
//!   slab     the resident terrain, one record a page. Not a pool: a page's
//!            record is its own coordinate reduced modulo the slab's axis
//!            (module/world/terrain/slab.rs), so nothing here is allocated,
//!            asked for or refused.
//!   heap     everything else, ascending from the page past the slab to the
//!            end of the memory. One allocator answers all of it — a slab of
//!            size classes over a buddy of pages (xpute ds/slab.rs over
//!            ds/buddy_tree.rs) — so a band's lanes, a chunk mesh and a
//!            `String` all come out of the same range and give back into it.
//!
//! Where each lands, at the two memories the shell reserves. Everything but
//! the heap is the same on both: the slab is the terrain a load radius
//! reaches, not a share of what the device happens to have.
//!
//!              offset         end            phone        desktop
//!   module     0              64 MiB         64 MiB       64 MiB
//!   slab       64 MiB         260.432 MiB    196.43 MiB   196.43 MiB
//!   (no one's) 260.432 MiB    260.434 MiB    1,504 B      1,504 B
//!   heap       260.434 MiB    —              251.57 MiB   763.57 MiB
//!
//! A section begins on a page (PAGE_BYTES) and ends where its content does;
//! the next begins on the first page past that. Where a section ends is no
//! one's business but its own, so the slab is the terrain it holds and not a
//! byte more, and what is left over is under a page.

use xpute_core::sys::memory::Section;

use crate::global::Global;
use crate::module::world::terrain::slab::SLAB_BYTES;

/// The memory the shell reserves, which is what the map is cut from. Not read
/// here — the kernel is handed what it was given — but written down so the two
/// sides say the same thing, and checked below against what the terrain takes.
pub const PHONE_BYTES: u32 = 512 << 20;
pub const DESKTOP_BYTES: u32 = 1024 << 20;

/// The unit the reservation is rounded down to, and where the fixed sections
/// are placed. Nothing cuts regions any more — the heap's allocator answers
/// for its own range — so this is a placement grid and nothing else.
pub const REGION_BYTES: u32 = 1 << 26;

/// The module's own section, from address 0: the stack, the rings and the
/// module's data. Two regions that grow without moving each other — the rings
/// have everything under pxl-wasm build.rs's `--global-base` and the data
/// everything over it — so this is the sum of the two and not a number either
/// of them has to fit alone.
pub const MODULE_BYTES: u32 = 64 << 20;

/// What a section begins on, and the smallest block the heap's buddy hands
/// out — the same 4 KiB as xpute `ds/buddy_tree.rs`'s MIN_ALLOC, for the same
/// reason rather than by coincidence
/// (issue/20260916-the-page-table-the-pool-did-not-need.md). A buddy block
/// lands at `HEAP_AT + k * its own size`, so a heap that begins on a smaller
/// boundary hands out pages that are not pages.
pub const PAGE_BYTES: u32 = 1 << 12;

const _: () = assert!(
    PAGE_BYTES as usize == xpute_core::ds::buddy_tree::MIN_ALLOC,
    "the heap's smallest block is the page the sections are laid out on"
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub slab: Section,
    pub heap: Section,
}

/// The map over a memory of `bytes`: the module's own section, then the slab,
/// each beginning on a page and ending where its content does, and the heap
/// from the page past the last of them to the end of the memory, ascending.
///
/// Which is the whole of the rule. What is fixed is settled at boot and never
/// moves; the heap is everything that is not, it starts where they stop, and
/// it is the only part of the map whose size is what the device happened to
/// give rather than what the content needs.
pub fn lay_out(bytes: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    let slab = Section {
        offset: MODULE_BYTES,
        bytes: SLAB_BYTES,
        region_bytes: SLAB_BYTES,
    };
    assert!(
        HEAP_AT + PAGE_BYTES <= regions * REGION_BYTES,
        "the memory is too small for the terrain the load radius reaches: the slab is {SLAB_BYTES} bytes and the heap begins past it"
    );
    Map {
        slab,
        heap: Section {
            offset: HEAP_AT,
            bytes: regions * REGION_BYTES - HEAP_AT,
            region_bytes: PAGE_BYTES,
        },
    }
}

/// Where the last fixed section ends, and where the heap begins past it. The
/// slab is the terrain the load radius reaches, which is not a round number of
/// anything, so the heap starts at the first page boundary past it and the
/// bytes between — under a page — are no one's.
///
/// `HEAP_AT` is a constant rather than a field of the map because the heap is
/// wanted before the map is: the module's global allocator is asked for its
/// first block by whatever runs first, which is well before anything has laid
/// the memory out.
const SLAB_END: u32 = MODULE_BYTES + SLAB_BYTES;
pub const HEAP_AT: u32 = SLAB_END.div_ceil(PAGE_BYTES) * PAGE_BYTES;

const _: () = assert!(
    MODULE_BYTES.is_multiple_of(PAGE_BYTES) && REGION_BYTES.is_multiple_of(PAGE_BYTES) && HEAP_AT.is_multiple_of(PAGE_BYTES),
    "a section begins on a page, so every block in it begins on one"
);

const _: () = assert!(
    HEAP_AT + PAGE_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the terrain the load radius reaches and a page of heap past it"
);

static MAP: Global<Option<Map>> = Global::new();

/// Records the map the module laid its memory out in.
pub fn set_map(map: Map) {
    MAP.set(Some(map));
}

/// The map, once the module has laid its memory out; none under a native
/// test, which has no sections.
pub fn map() -> Option<Map> {
    *MAP.get(|| None)
}

#[cfg(test)]
#[path = "memory.test.rs"]
mod test;
