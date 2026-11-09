// pxl-spec/space/coord/zoom.rs

//! Every size is a zoom: X goes once around the world in 2^24 units, so a thing as wide as
//! the world divided by 2^z is at zoom z. A cell's, a square's and a tile's size are each
//! named by this one number; a bigger zoom is finer, and one zoom finer is half as wide.

use super::aabb::Aabb;
use super::q3::{FRAC_W, Q3};
use super::world;
use super::world::HEIGHT_BITS;
use xpute_kit::math::scalar::round;

/// A zoom; every size is named by one.
pub type Zoom = u32;

/// The zoom of a unit: 1u is a z24 cell's side, and X goes once around the world in 2^24
/// units.
pub const UNIT_ZOOM: Zoom = 24;

/// The coarsest cube: a tile at this zoom is as wide as the world is tall, so the pyramid is
/// alike on every axis from here on finer, and 3D begins. A tile at this zoom with the
/// world's whole height over it is a **shard**, the unit in which the world's data is
/// ingested, baked and kept; what is ingested of one is the shard and its apron.
pub const SHARD_ZOOM: Zoom = UNIT_ZOOM - HEIGHT_BITS;

/// Where Z takes its first digit: the cubes under a shard, eight of them, the
/// world's height split at sea level.
pub const CUBE_ZOOM: Zoom = SHARD_ZOOM + 1;

/// The leaves' zoom: the finest tile any layer is kept in, a page of the
/// world 2^15 round.
pub const LEAF_ZOOM: Zoom = 15;

/// The top: the coarsest tile any layer is kept in. A layer is kept in tiles from this zoom
/// down to the leaves, and a source declares its own at this zoom or finer (its
/// declaration's `min_zoom`).
pub const MIN_ZOOM: Zoom = 4;

/// A tile is 2^9 cells a side, so a leaf tile's cells are at this zoom, the ground's finest
/// cell (`lattice::GROUND`).
pub const LEAF_CELL_ZOOM: Zoom = LEAF_ZOOM + 9;

/// The finest a point resolves: a fraction's step.
pub const FINEST_ZOOM: Zoom = UNIT_ZOOM + FRAC_W;

/// log2 of `zoom`'s side in units, negative where finer than a unit. Only `space/` uses it,
/// so the lattice is shifted here and nowhere else.
pub(in crate::space) const fn shamt(zoom: Zoom) -> i32 {
    UNIT_ZOOM as i32 - zoom as i32
}

/// `zoom`'s side in units.
pub const fn units(zoom: Zoom) -> f64 {
    let s = shamt(zoom);
    if s >= 0 {
        (1u64 << s) as f64
    } else {
        1.0 / (1u64 << -s) as f64
    }
}

/// How many `inner` cells stand across one `outer`, which is no finer.
pub const fn cnt(inner: Zoom, outer: Zoom) -> u32 {
    if inner < outer {
        panic!("an outer zoom finer than the inner")
    }
    1 << (inner - outer)
}

/// Index `v` along an axis at `zoom`, as an index at `to`. Toward a finer zoom it is the
/// first of the finer cells inside it; toward a coarser one, the coarser cell it lies in.
pub const fn idx_at(zoom: Zoom, v: i32, to: Zoom) -> i32 {
    if to >= zoom {
        v << (to - zoom)
    } else {
        v >> (zoom - to)
    }
}

/// The offset that turns Y's index at `zoom` into a count from the world's south edge: 0 at
/// zoom 0, and half the plane's nodes at every finer zoom.
pub const fn y_start(zoom: Zoom) -> i32 {
    if zoom == 0 {
        0
    } else {
        (1u32 << (zoom - 1)) as i32
    }
}

/// The offset that turns Z's index at `zoom` into a count from the world's floor: 0 down to
/// a shard's zoom, where one cube spans the world's height, and half the height's nodes at
/// every finer zoom.
pub const fn z_start(zoom: Zoom) -> i32 {
    if zoom <= SHARD_ZOOM {
        0
    } else {
        (1u32 << (zoom - SHARD_ZOOM - 1)) as i32
    }
}

/// The zoom whose side is `units`, a power of two.
pub fn of_side(units: f64) -> Zoom {
    (UNIT_ZOOM as i32 - round(libm::log2(units)) as i32) as Zoom
}

/// The zoom of the widest power-of-two side no wider than `units`, signed:
/// past the world it is below zero.
pub fn within(units: f64) -> i32 {
    UNIT_ZOOM as i32 - libm::floor(libm::log2(units)) as i32
}

/// A floor on a signed axis.
pub const fn idx(zoom: Zoom, units: i64) -> i64 {
    let s = shamt(zoom);
    if s >= 0 {
        units >> s
    } else {
        units << -s
    }
}

/// The cell at `zoom` a position stands in, a floor on each axis, Y and Z's
/// from the world's edge (`y_start`, `z_start`): one above where an axis has no digit.
pub fn idx_of(zoom: Zoom, at: &Q3) -> [i32; 3] {
    let shift = FRAC_W as i32 + shamt(zoom);
    // Shifted as an i64, as wide as zoom 0's 32 bits; the index is no wider
    // than the position it is of.
    let floor = |k: usize, start: i32| {
        let edge = 1i64 << (world::EXTENT[k] - 1 + FRAC_W);
        (((i64::from(at[k]) + edge) >> shift) as i32).wrapping_sub(start)
    };
    [
        (i64::from(at[0]) >> shift) as i32,
        floor(1, y_start(zoom)),
        floor(2, z_start(zoom)),
    ]
}

/// Whether axis `axis` has a digit at `zoom`, so that a node there stands for part of it, not
/// for the world's whole extent of it. X wraps around.
pub const fn has_digit(axis: usize, zoom: Zoom) -> bool {
    axis == 0 || zoom > UNIT_ZOOM - world::EXTENT[axis]
}

/// `idx_of`'s inverse, to the cell's base.
pub fn base_of(zoom: Zoom, idx: [i32; 3]) -> Q3 {
    let shift = FRAC_W as i32 + shamt(zoom);
    let [x, y, z] = idx.map(i64::from);
    world::at(x << shift, y << shift, z << shift)
}

/// The box the node `idx` of `zoom` stands for, the world's whole of an axis it
/// has no digit of.
pub fn cube(zoom: Zoom, idx: [i32; 3]) -> Aabb {
    let side = units(zoom);
    let mut b = world::BOUNDS;
    for (k, &v) in idx.iter().enumerate() {
        if has_digit(k, zoom) {
            b.min[k] = f64::from(v) * side;
            b.max[k] = b.min[k] + side;
        }
    }
    b
}

#[cfg(test)]
#[path = "zoom.test.rs"]
mod test;
