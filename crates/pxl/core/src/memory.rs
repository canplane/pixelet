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
//!              offset         end          phone        desktop
//!   module     0              32 MiB       32 MiB       32 MiB
//!   fixed      32 MiB         64 MiB       32 MiB       32 MiB
//!   slab       64 MiB         260.43 MiB   196.43 MiB   196.43 MiB
//!   (no one's) 260.43 MiB     320 MiB      59.57 MiB    59.57 MiB
//!
//! A section begins on a power-of-two boundary and ends where its content
//! does. Where a section ends is no one's business but its own: nothing starts
//! there — the mesh's first region is past the slab's end, not at it — so the
//! slab is the terrain it holds and not a byte more.
//!   mesh       320 MiB        —            192 MiB      704 MiB

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



/// The sections' pools, as the kernel holds them. The slab has none.
pub const FIXED_POOL: u32 = 0;
pub const MESH_POOL: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub slab: Section,
    pub mesh: Section,
}

/// The map over a memory of `bytes`: the module and the fixed section share
/// the first region, the slab is exactly the terrain it holds, and the mesh
/// takes every whole region past it.
///
/// The slab is the size of the set the load radius reaches and nothing more,
/// which is not a whole number of regions; the mesh's pool needs its range to
/// be whole regions (xpute ds/buddy.rs), so the bytes between the slab's end
/// and the mesh's first region are no one's. What they are is arithmetic:
/// with a memory sized to the slab they are none.
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
    let mesh_at = MESH_AT;
    assert!(
        mesh_at + REGION_BYTES <= regions * REGION_BYTES,
        "the memory is too small for the terrain the load radius reaches: the slab is {SLAB_BYTES} bytes and the mesh needs a region past it"
    );
    Map {
        fixed,
        slab,
        mesh: Section {
            offset: mesh_at,
            bytes: regions * REGION_BYTES - mesh_at,
            region_bytes: REGION_BYTES,
        },
    }
}

/// Where the slab ends, and where the mesh's first whole region begins past
/// it. The slab is the terrain the load radius reaches and not a round number
/// of regions, and the mesh's pool takes nothing but whole regions, so the
/// bytes between the two are no one's. They are the price of the slab being
/// the set rather than the set being the slab.
const SLAB_END: u32 = REGION_BYTES + SLAB_BYTES;
const MESH_AT: u32 = SLAB_END.div_ceil(REGION_BYTES) * REGION_BYTES;

const _: () = assert!(
    MODULE_BYTES.is_power_of_two() && REGION_BYTES.is_power_of_two() && MESH_AT.is_multiple_of(REGION_BYTES),
    "a section begins where a power of two does, so an address in it is an offset from a shift"
);

const _: () = assert!(
    MESH_AT + REGION_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the terrain the load radius reaches and a region of mesh past it"
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
