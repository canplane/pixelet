// pxl_core/memory.rs

//! The kernel's policy over the memory the central scheduler gives it: which
//! sections there are and how many regions each takes. Cutting the memory
//! into sections is the mechanism's (xpute_core::sys::memory); what a
//! section holds, and what "full" means in it, is decided here
//! (issue/20260913-sections-of-the-core-memory-and-what-full-means.md).
//!
//! Sections, in address order:
//!
//!   module   the module's stack and data, where a wasm32 linker puts them
//!            (`--stack-first`, then data), and its heap: pxl-wasm's global
//!            allocator, xpute's FixedHeap, over the rest of the section.
//!   fixed    what is allocated once and never freed: the rings, the
//!            renderer's blocks, the volumes' standing band stores. Refuses
//!            rather than spills, so a shortfall fails at boot. Where inside
//!            it each of those sits is that section's own affair — the map
//!            gives it a range and asks nothing more.
//!   slab     the resident terrain, one record a page. Not a pool: a page's
//!            record is its own coordinate reduced modulo the slab's axis
//!            (module/world/terrain/slab.rs), so nothing here is allocated,
//!            asked for or refused.
//!   mesh     chunk meshes, run sets, tier caches, bands — what churns as the
//!            view moves, and what the reclaim pass answers to.
//!
//! Where each lands, at the two memories the shell reserves. Everything but
//! the mesh is the same on both: the slab is the terrain a load radius
//! reaches, not a share of what the device happens to have.
//!
//!              offset         end            phone        desktop
//!   module     0              32 MiB         32 MiB       32 MiB
//!   fixed      32 MiB         64 MiB         32 MiB       32 MiB
//!   slab       64 MiB         260.432 MiB    196.43 MiB   196.43 MiB
//!   (no one's) 260.432 MiB    260.434 MiB    1,504 B      1,504 B
//!   mesh       260.434 MiB    —              251.57 MiB   763.57 MiB
//!
//! A section begins on a page (PAGE_BYTES) and ends where its content does;
//! the next begins on the first page past that. Where a section ends is no
//! one's business but its own, so the slab is the terrain it holds and not a
//! byte more, and what is left over is under a page.
//!
//! What a section does inside its range is its own affair too. The mesh's pool
//! cuts 64 MB regions and can only cut whole ones, so the tail of its range
//! past the last it can cut — 59.57 MiB at that region size — goes unused; that
//! is the pool's number to answer for, not the map's.

use xpute_core::sys::memory::Section;

use crate::global::Global;
use crate::module::world::terrain::slab::SLAB_BYTES;

/// The memory the shell reserves, which is what the map is cut from. Not read
/// here — the kernel is handed what it was given — but written down so the two
/// sides say the same thing, and checked below against what the terrain takes.
pub const PHONE_BYTES: u32 = 512 << 20;
pub const DESKTOP_BYTES: u32 = 1024 << 20;

/// The unit sections are counted in, and the mesh pool's largest block.
pub const REGION_BYTES: u32 = 1 << 26;

/// The module's own section, from address 0.
pub const MODULE_BYTES: u32 = 32 << 20;

/// The fixed section's pool cuts what is left of the first region in 16 MB
/// regions, so the map stays region-aligned where the pools need it.
pub const FIXED_REGION_BYTES: u32 = 16 << 20;

/// The smallest block a section's pool hands out.
pub const MIN_BLOCK_BYTES: u32 = 1 << 12;

/// What a section begins and ends on. The same 4 KiB as MIN_BLOCK_BYTES, for
/// the same reason rather than by coincidence: a pool's block lands at
/// `section start + n * region_bytes + k * MIN_BLOCK_BYTES`, so a section that
/// begins on a smaller boundary hands out blocks that do not. Nothing asks for
/// more — a region size is what a pool cuts inside its own range, not where the
/// range may begin (xpute ds/buddy.rs's `_locate` reads a region's index off
/// the address relative to the range's start).
pub const PAGE_BYTES: u32 = 1 << 12;



#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub slab: Section,
    pub heap: Section,
}

/// The map over a memory of `bytes`: the fixed sections first, each beginning
/// on a page and ending where its content does, and the heap from the page
/// past the last of them to the end of the memory, ascending.
///
/// Which is the whole of the rule. What is fixed is settled at boot and never
/// moves; the heap is everything that is not, it starts where they stop, and
/// it is the only part of the map whose size is what the device happened to
/// give rather than what the content needs.
pub fn lay_out(bytes: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    let fixed = Section {
        offset: MODULE_BYTES,
        bytes: REGION_BYTES - MODULE_BYTES,
        region_bytes: FIXED_REGION_BYTES,
    };
    let slab = Section {
        offset: REGION_BYTES,
        bytes: SLAB_BYTES,
        region_bytes: SLAB_BYTES,
    };
    assert!(
        HEAP_AT + PAGE_BYTES <= regions * REGION_BYTES,
        "the memory is too small for the terrain the load radius reaches: the slab is {SLAB_BYTES} bytes and the heap begins past it"
    );
    Map {
        fixed,
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
const SLAB_END: u32 = REGION_BYTES + SLAB_BYTES;
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
