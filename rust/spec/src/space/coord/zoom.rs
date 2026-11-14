// pxl-spec/space/coord/zoom.rs

//! Every size is a zoom: X goes once around the world in 2^24 units, so a thing as wide as
//! the world divided by 2^z is at zoom z. A cell's, a square's and a tile's size are each
//! named by this one number; a bigger zoom is finer, and one zoom finer is half as wide.

use super::q3::FRAC_W;
use xpute_kit::math::scalar::round;

/// A zoom; every size is named by one.
pub type Zoom = u32;

/// The zoom of a unit: 1u is a z24 cell's side, and X goes once around the world in 2^24
/// units.
pub const UNIT_ZOOM: Zoom = 24;

/// The world's extent an axis, in bits of a unit:
///
/// - X, 24: a unit is a z24 cell's side, so a lap is 2^24 units (`UNIT_ZOOM`), and
///   a web map's z16 pixel is a z24 cell;
/// - Y, 24: the Mercator plane is a square;
/// - Z, 16: ±2^15 units of height, as a tile's elevation samples and the cells' elevations
///   worked out from them are each an `i16` in ground cells, a ground cell being a unit
///   (`bake/constant.rs`, `ground.rs` `ele_at`).
pub const EXTENT: [u32; 3] = [UNIT_ZOOM, UNIT_ZOOM, HEIGHT_BITS];

/// The world's height in units, as bits: Z spans 2^16 of them about sea level.
pub const HEIGHT_BITS: u32 = 16;

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

/// How many `inner` cells stand across one `outer`, which is no finer: every caller passes such
/// a pair, so a finer outer is unreachable, and in a constant fails the build.
pub const fn cnt(inner: Zoom, outer: Zoom) -> u32 {
    if inner < outer {
        unreachable!()
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

#[cfg(test)]
#[path = "zoom.test.rs"]
mod test;
