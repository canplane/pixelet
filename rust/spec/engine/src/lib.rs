// pxl-engine/lib.rs

//! gpu-portable: the rules that the CPU runs and the GPU's WGSL is written to. A module headed
//! `gpu-portable` computes what the CPU and the GPU must compute alike, in `core` alone.
//!
//! The crate holds the zooms and the lattices of cells on them, a node's seed, the value
//! space, the ground and the service area it ends at, the ray's descent, and the samples'
//! stops as the boxes a raster draws. It asks the content it draws for that content's rules
//! (`content`). pxl-spec takes the crate as its own. pxl-render's WGSL passes are written by
//! hand to it, their layouts are generated from it (pxl-render build.rs), and a test holds
//! each pass word for word to it (pxl-render tests/).
//!
//! Nothing here can fail a check at run time: every index stays inside its array, no divisor
//! is zero, and no range's bounds cross.

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
pub mod way_tree;
pub mod world;

/// A zoom; every size is named by one (pxl-spec space/coord/zoom.rs).
pub type Zoom = u32;

/// The zoom of a unit: one unit is the side of a z24 cell, and X goes once around the world
/// in 2^24 units (pxl-spec space/coord/zoom.rs).
pub const UNIT_ZOOM: Zoom = 24;

/// The finest tile zoom (pxl-spec space/layer/quadrant.rs).
pub const LEAF_ZOOM: Zoom = 15;

/// The coarsest tile zoom, the top of the pyramid. A layer is kept in tiles from this zoom
/// down to the leaves, and a source declares its own at this zoom or finer (its
/// declaration's `min_zoom`).
pub const MIN_ZOOM: Zoom = 4;

/// A tile is 2^9 cells a side, so a leaf tile's cells are at this zoom, the ground's finest
/// cell (`lattice::GROUND`).
pub const LEAF_CELL_ZOOM: Zoom = LEAF_ZOOM + 9;

/// Sea level, in leaf cells: the height heights are counted from, where the
/// water stands and under which the Earth past the service area is rock.
pub const SEA_LEVEL: i32 = 0;

/// `m` ground meters in leaf cells, rounded half to even, where `per_m` is the leaf cells a
/// ground meter at that place (pxl-spec procedural.rs `Frame::per_m`). Every length given in
/// meters becomes cells this way, in the bake and in the client alike.
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
/// `maxComputeWorkgroupsPerDimension` at its default. The page's device keeps the default,
/// because it asks for no limits, and a native device's adapter allows at least as many.
/// Every count a pass writes for an indirect dispatch stays under it, because the room the
/// pass lists into is no larger.
pub const WORKGROUPS_CAP: u32 = 65_535;

/// A storage buffer's view is bound at a multiple of this many words: WebGPU's
/// `minStorageBufferOffsetAlignment` at its default, 256 bytes, the most any device asks.
pub const STORAGE_ALIGN_WORDS: u32 = 256 / 4;

/// The coarsest cube: a tile at this zoom is as wide as the world is tall, so the pyramid is
/// alike on every axis from here on finer, and 3D begins. A tile at this zoom with the
/// world's whole height over it is a **shard**, the unit in which the world's data is
/// ingested, baked and kept.
pub const SHARD_ZOOM: Zoom = UNIT_ZOOM - HEIGHT_BITS;

/// Element `i` of `s`, a buffer the host wrote or a table, read so that no read aborts:
/// outside the slice it is `T::default()`, which means nothing held, void or zero. This is
/// the one rule for what lies outside a buffer. A host that indexes past what it wrote breaks
/// a contract, which the CPU's debug builds catch.
pub fn at<T: Copy + Default>(s: &[T], i: u32) -> T {
    // The check runs only in debug builds, and the shader crate is never built in debug.
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

/// Component `a` of an axis triple, chosen by comparison rather than indexed, so no read
/// aborts. Past the third component, it is the third.
pub fn axis<T: Copy>(v: [T; 3], a: u32) -> T {
    if a == 0 {
        v[0]
    } else if a == 1 {
        v[1]
    } else {
        v[2]
    }
}

/// The bits a zoom takes wherever it is packed into a word; every zoom fits.
pub const ZOOM_BITS: u32 = 6;
const _: () = assert!(FINEST_ZOOM < 1 << ZOOM_BITS);

/// Index `v` along an axis at `zoom`, as an index at `to`. Toward a finer zoom it is the
/// first of the finer cells inside it; toward a coarser one, the coarser cell it lies in.
pub const fn idx_at(zoom: Zoom, v: i32, to: Zoom) -> i32 {
    if to >= zoom {
        v << (to - zoom)
    } else {
        v >> (zoom - to)
    }
}

/// X at `zoom` as the world names it, wrapped around the world into the range from minus half
/// the count to under half (pxl-spec space/coord/world.rs `wrap_idx`).
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
