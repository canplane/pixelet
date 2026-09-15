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
//!            rather than spills, so a shortfall fails at boot.
//!   page     resident terrain and POI pages, bounded by the count kept.
//!   mesh     chunk meshes, run sets, tier caches, bands — what churns as the
//!            view moves, and what the reclaim pass answers to.

use xpute_core::sys::memory::{lay_out as cut, Section};

use crate::global::Global;

/// The unit sections are counted in, and a page or mesh pool's largest block.
pub const REGION_BYTES: u32 = 1 << 26;

/// The module's own section, from address 0.
pub const MODULE_BYTES: u32 = 32 << 20;

/// The fixed section's pool cuts what is left of the first region in 16 MB
/// regions, so the map stays region-aligned where the pools need it.
pub const FIXED_REGION_BYTES: u32 = 16 << 20;

/// The smallest block a section's pool hands out.
pub const MIN_BLOCK_BYTES: u32 = 1 << 12;

/// The submission and completion rings, at fixed places the host finds from
/// the boot block, and the completion ring's spill, which only the kernel
/// reads: the first three blocks of the fixed section, taken at boot before
/// anything else and never given back.
pub const RING_BYTES: u32 = 1 << 20;
pub const RING_SQ_AT: u32 = MODULE_BYTES;
pub const RING_CQ_AT: u32 = MODULE_BYTES + RING_BYTES;
pub const RING_SPILL_AT: u32 = MODULE_BYTES + 2 * RING_BYTES;

/// The sections' pools, as the kernel holds them.
pub const FIXED_POOL: u32 = 0;
pub const PAGE_POOL: u32 = 1;
pub const MESH_POOL: u32 = 2;

/// The most regions the page section takes: three hold every page kept
/// resident (module/world/terrain/config.rs derives the count from this
/// section); past them, every region is the mesh section's churn.
const PAGE_REGIONS_MAX: u32 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub page: Section,
    pub mesh: Section,
}

/// The map over a memory of `bytes`: one fixed region, the page section up to
/// PAGE_REGIONS_MAX with a region left over, and the rest mesh — eight
/// regions are 1, 3 and 4, four are 1, 2 and 1.
pub fn lay_out(bytes: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    assert!(regions >= 3, "the kernel needs a region for each section");
    let page = (regions - 2).min(PAGE_REGIONS_MAX);
    let mut s = [Section::default(); 3];
    cut(bytes, MODULE_BYTES, REGION_BYTES, FIXED_REGION_BYTES, &[1, page], &mut s);
    Map {
        fixed: s[0],
        page: s[1],
        mesh: s[2],
    }
}

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
