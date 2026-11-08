// pxl-engine/lib.rs

//! gpu-portable: the rules the CPU runs and the GPU's WGSL is written to.
//!
//! What pixelet's spec computes the same on the CPU and on the GPU, in
//! `core` alone: the zooms and the lattices of cells on them, a node's seed,
//! the value space, the ground and the service area it ends at, the ray's
//! descent, and the samples' stops as the boxes a raster draws, asking the
//! content it draws its rules (`content`).
//! pxl-spec takes it as its own; pxl-render's WGSL passes are written by hand
//! to it, their layouts generated from it (pxl-render build.rs), each held word
//! for word to it by a test (pxl-render tests/).
//!
//! No check here fails: an index stays in its array, a divisor is not zero,
//! a range's bounds do not cross.

#![no_std]

pub mod answer;
pub mod area;
pub mod beam;
pub mod cast;
pub mod clipmap;
pub mod content;
pub mod count;
pub mod eye;
pub mod face;
pub mod fill;
pub mod grid;
pub mod ground;
pub mod lattice;
pub mod light;
pub mod node_cache;
pub mod object;
pub mod param;
pub mod raster;
pub mod ray;
pub mod record;
pub mod seed;
pub mod support;
pub mod value;
pub mod way;
pub mod world;

/// Every size is a zoom (pxl-spec space/coord/zoom.rs).
pub type Zoom = u32;

/// A unit's: 1u is a z24 cell's side, X going once round the world in 2^24
/// of them (pxl-spec space/coord/zoom.rs).
pub const UNIT_ZOOM: Zoom = 24;

/// The finest tile zoom (pxl-spec space/layer/quadrant.rs).
pub const LEAF_ZOOM: Zoom = 15;

/// The coarsest tile zoom, the pyramid's top: what a layer is kept in runs
/// from it to the leaves, and a source declares its own top at it or finer
/// (its `top_zoom`).
pub const TOP_ZOOM: Zoom = 4;

/// A tile is 2^9 cells a side, so a leaf tile's cells are at this zoom: the
/// ground's finest cell (`lattice::GROUND`).
pub const LEAF_CELL_ZOOM: Zoom = LEAF_ZOOM + 9;

/// Sea level, in leaf cells: the height heights are counted from, where the
/// water stands and under which the Earth past the service area is rock.
pub const SEA_LEVEL: i32 = 0;

/// `m` ground meters in leaf cells, `per_m` leaf cells a ground meter where
/// they lie (pxl-spec procedural.rs `Frame::per_m`), rounded half to even:
/// what every length given in meters is, to the bake and the client alike.
pub fn cells_of_m(m: f32, per_m: f32) -> i32 {
    libm::rintf(m * per_m) as i32
}

/// A Q3 position's fraction bits: a unit is 256 steps (pxl-spec
/// space/coord/q3.rs).
pub const FRAC_W: u32 = 8;

/// The finest zoom a position names: a step of a Q3.
pub const FINEST_ZOOM: Zoom = UNIT_ZOOM + FRAC_W;

/// The world's height in units, as bits: Z spans 2^16 of them about sea
/// level (pxl-spec space/coord/world.rs `EXTENT`).
pub const HEIGHT_BITS: u32 = 16;

/// The most workgroups a dispatch takes on an axis: WebGPU's
/// `maxComputeWorkgroupsPerDimension` at its default, which the page's
/// device keeps, as it asks for no limits; a native device's adapter allows
/// at least as many. Each count a pass writes for an indirect dispatch is
/// held under it by the room it lists into.
pub const WORKGROUPS_CAP: u32 = 65_535;

/// The words a storage buffer's view is bound at a multiple of: WebGPU's
/// `minStorageBufferOffsetAlignment` at its default, 256 bytes, the most any
/// device asks.
pub const STORAGE_ALIGN_WORDS: u32 = 256 / 4;

/// The coarsest cube: a tile here is as wide as the world is tall, so the
/// pyramid is alike on every axis from here finer, and 3D begins. A tile of
/// it with the world's whole height over it is a **shard**, the unit the
/// world's data is ingested, baked and kept in.
pub const SHARD_ZOOM: Zoom = UNIT_ZOOM - HEIGHT_BITS;

/// Element `i` of `s`, a buffer the host wrote or a table, read totally:
/// what lies outside it is `T::default()` — nothing held, void, zero — so
/// no read aborts. The one fact of what is outside a buffer; a host that
/// indexes past what it wrote breaks a contract, which the CPU's debug
/// builds catch.
pub fn at<T: Copy + Default>(s: &[T], i: u32) -> T {
    // Debug builds only, as the shader crate's never are.
    #[cfg(debug_assertions)]
    assert!(
        (i as usize) < s.len(),
        "read past a buffer: {i} of {}",
        s.len()
    );
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked(i as usize) }
    } else {
        T::default()
    }
}

/// Component `a` of an axis triple, chosen rather than indexed, so no read
/// aborts: past the third, the third.
pub fn axis<T: Copy>(v: [T; 3], a: u32) -> T {
    if a == 0 {
        v[0]
    } else if a == 1 {
        v[1]
    } else {
        v[2]
    }
}

/// A zoom's bits wherever one is packed in a word: every zoom fits them.
pub const ZOOM_BITS: u32 = 6;
const _: () = assert!(FINEST_ZOOM < 1 << ZOOM_BITS);

/// Index `v` along an axis at `zoom` as an index at `to`: the first of the
/// finer that lie in it, or the coarser it lies in, the floor.
pub const fn idx_at(zoom: Zoom, v: i32, to: Zoom) -> i32 {
    if to >= zoom {
        v << (to - zoom)
    } else {
        v >> (zoom - to)
    }
}

/// X named as the world names it at `zoom`, X going round: from minus half
/// its count to under half (pxl-spec space/coord/world.rs `wrap_idx`).
pub fn wrap_idx(zoom: Zoom, x: i32) -> i32 {
    match zoom {
        0 => 0,
        z => x.wrapping_shl(32 - z) >> (32 - z),
    }
}

/// How many `inner` cells stand across one `outer`, which is no finer.
pub const fn cnt(inner: Zoom, outer: Zoom) -> u32 {
    if inner < outer {
        panic!("an outer zoom finer than the inner")
    }
    1 << (inner - outer)
}
