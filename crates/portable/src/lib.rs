// pxl-portable/lib.rs

//! gpu-portable: one source for the CPU and, through rust-gpu, WGSL.
//!
//! What pixelet's spec computes the same on the CPU and on the GPU, in
//! `core` alone: the zooms and the lattices of cells on them, a node's seed
//! and the caves, the ground and the service area it ends at, the decor the
//! ground grows, and the ray's descent. pxl-spec takes it as its own, and a shader crate depends on it as
//! it is.

#![no_std]

pub mod area;
pub mod cast;
pub mod caves;
#[path = "cube.spec.rs"]
pub mod cube;
#[path = "decor.spec.rs"]
pub mod decor;
pub mod eye;
pub mod fill;
pub mod ground;
pub mod lattice;
pub mod placed;
pub mod ray;
pub mod scatter;
pub mod ways;
pub mod window;

/// Every size is a zoom (pxl-spec space/coord/zoom.rs).
pub type Zoom = u32;

/// A unit's: 1u is a z24 cell's side, X going once round the world in 2^24
/// of them (pxl-spec space/coord/zoom.rs).
pub const UNIT_ZOOM: Zoom = 24;

/// The finest tile zoom (pxl-spec space/layer/tile.rs).
pub const LEAF_ZOOM: Zoom = 15;

/// A tile is 2^9 cells a side, so a leaf tile's cells are at this zoom: the
/// ground's finest cell (`lattice::GROUND`).
pub const LEAF_CELL: Zoom = LEAF_ZOOM + 9;

/// Sea level, in leaf cells: the height heights are counted from, where the
/// water stands and under which the Earth past the service area is rock.
pub const SEA_LEVEL: i32 = 0;

/// `m` ground metres in leaf cells, `per_m` leaf cells a ground metre where
/// they lie (pxl-spec procedural.rs `Frame::per_m`), rounded half to even:
/// what every length given in metres is, to the bake and the client alike.
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

/// The coarsest cube: a tile here is as wide as the world is tall, so the
/// pyramid is alike on every axis from here finer, and 3D begins. A tile of
/// it with the world's whole height over it is a **shard**, the unit the
/// world's data is ingested, baked and kept in.
pub const SHARD_ZOOM: Zoom = UNIT_ZOOM - HEIGHT_BITS;

/// A chunk, what is built and drawn as one, is 2^CHUNK_STEPS of the camera
/// grid's cells a side (pxl-spec space/camera/grid.rs).
pub const CHUNK_STEPS: Zoom = 5;

/// Element `i` of `s`, a buffer the host wrote or a table, read totally:
/// what lies outside it is `T::default()` — nothing held, void, zero — so
/// no read aborts, and a GPU's code holds no path for one. The one fact of
/// what is outside a buffer; a host that indexes past what it wrote breaks
/// a contract, which the CPU's debug builds still catch.
pub fn at<T: Copy + Default>(s: &[T], i: u32) -> T {
    // Debug builds only, as the shader crate's never are.
    #[cfg(debug_assertions)]
    assert!((i as usize) < s.len(), "read past a buffer: {i} of {}", s.len());
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
pub const fn index_at(zoom: Zoom, v: i32, to: Zoom) -> i32 {
    if to >= zoom {
        v << (to - zoom)
    } else {
        v >> (zoom - to)
    }
}

/// X named as the world names it at `zoom`, X going round: from minus half
/// its count to under half (pxl-spec space/coord/world.rs `wrap_index`).
pub fn wrap_index(zoom: Zoom, x: i32) -> i32 {
    match zoom {
        0 => 0,
        z => x.wrapping_shl(32 - z) >> (32 - z),
    }
}

/// How many `inner` cells stand across one `outer`, which is no finer.
pub const fn count(inner: Zoom, outer: Zoom) -> u32 {
    if inner < outer {
        panic!("an outer zoom finer than the inner")
    }
    1 << (inner - outer)
}
