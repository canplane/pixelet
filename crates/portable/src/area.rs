// pxl-portable/area.rs

//! gpu-portable: where the world's data ends, the service area — the shards
//! a source serves — and the ground's own rule at its edge (pxl-spec
//! block/limit.rs): past it, the Earth's ground, solid to sea level; inside
//! it, a leaf column with ground beside one past the area is bedrock up to
//! its own face, so the wall's outer face stands on the area's edge at every
//! zoom. The area runs on shards' edges, which no node a shard or finer
//! straddles.

use crate::cube::{sentinel, Ground};
use crate::{wrap_index as wrap, Zoom, LEAF_CELL, LEAF_ZOOM, SEA_LEVEL, SHARD_ZOOM};

const VOID: u32 = sentinel::void as u32;
const UNKNOWN: u32 = sentinel::unknown as u32;

/// The shards a source serves: on the CPU its declaration, on a GPU a
/// buffer of it.
pub trait Area {
    /// Whether the area ends at all: before a source is declared, nothing
    /// is past it.
    fn ends(&self) -> bool;
    fn shards(&self) -> u32;
    /// Shard `i`: its tile's zoom and index, X named as the world names it.
    fn shard(&self, i: u32) -> (Zoom, [i32; 2]);
}

/// Whether the leaf tile `leaf` is past the area: it ends, and no served
/// shard is over the tile.
pub fn past(area: &impl Area, leaf: [i32; 2]) -> bool {
    if !area.ends() {
        return false;
    }
    let mut i = 0;
    while i < area.shards() {
        let (z, at) = area.shard(i);
        if z <= LEAF_ZOOM {
            let s = LEAF_ZOOM - z;
            if leaf[0] >> s == at[0] && leaf[1] >> s == at[1] {
                return false;
            }
        }
        i += 1;
    }
    true
}

/// The leaf tile holding leaf cell `cell`.
fn tile_of(cell: [i32; 2]) -> [i32; 2] {
    let s = LEAF_CELL - LEAF_ZOOM;
    [wrap(LEAF_ZOOM, cell[0] >> s), cell[1] >> s]
}

/// A node's index along an axis in leaf cells, the floor where it is finer.
fn in_leaf_cells(zoom: Zoom, v: i32) -> i32 {
    if zoom <= LEAF_CELL {
        v << (LEAF_CELL - zoom)
    } else {
        v >> (zoom - LEAF_CELL)
    }
}

/// The leaf tile holding the node's middle.
pub fn tile_at(zoom: Zoom, c: [i32; 2]) -> [i32; 2] {
    if zoom < LEAF_ZOOM {
        let s = LEAF_ZOOM - zoom;
        [wrap(LEAF_ZOOM, (c[0] << s) + (1 << (s - 1))), (c[1] << s) + (1 << (s - 1))]
    } else {
        let s = zoom - LEAF_ZOOM;
        [c[0] >> s, c[1] >> s]
    }
}

/// Whether the service area's wall may stand in the node `(zoom, c)` asked
/// at `cell`: only a leaf's cells or finer are walled, a leaf cell with
/// ground beside one past the area. A node wider than a shard may hold the
/// area's edge; one no wider is asked at its corners a leaf cell out.
pub fn walled(area: &impl Area, cell: Zoom, (zoom, c): (Zoom, [i32; 2])) -> bool {
    if cell < LEAF_CELL {
        return false;
    }
    if zoom < SHARD_ZOOM {
        return true;
    }
    let lo = [in_leaf_cells(zoom, c[0]) - 1, in_leaf_cells(zoom, c[1]) - 1];
    let hi = [in_leaf_cells(zoom, c[0] + 1) + 1, in_leaf_cells(zoom, c[1] + 1) + 1];
    past(area, tile_of(lo)) || past(area, tile_of([hi[0], lo[1]])) || past(area, tile_of([lo[0], hi[1]])) || past(area, tile_of(hi))
}

/// The leaf cells about the leaf cell holding the block `b` at `cell`, a
/// leaf's or finer, that its eight neighbours lie in: bit `(dy + 1)·3 +
/// (dx + 1)` for the leaf cell `dx, dy` from its own, its own never. A block
/// within its leaf cell touches none; one on a leaf's edge or corner, the
/// cells across them; at the leaves, all eight. Whether the block has
/// ground beside one gone (`wall`) is whether any of these is.
pub fn ring(cell: Zoom, b: [i32; 2]) -> u32 {
    let m = (1 << (cell - LEAF_CELL)) - 1;
    // Per axis, the leaf offsets its neighbours reach: 0 always, -1 at the
    // leaf cell's low edge, +1 at its high one.
    let reach = |v: i32| 0b010 | ((v & m == 0) as u32) | ((v & m == m) as u32) << 2;
    let (x, y) = (reach(b[0]), reach(b[1]));
    let row = |o: u32| if y >> o & 1 == 1 { x << (3 * o) } else { 0 };
    (row(0) | row(1) | row(2)) & !(1 << 4)
}

/// A cell's look with the area's edge laid (pxl-spec block/sample.rs
/// `Grid::wall_off`), `gone` where the cell is past the area or nothing is
/// known of it, `beside` where one of the eight about it is: gone, void;
/// at a leaf's cells or finer, ground beside one gone is the wall, bedrock.
pub fn wall(look: u32, cell: Zoom, gone: bool, beside: bool) -> u32 {
    if gone {
        VOID
    } else if cell >= LEAF_CELL && Ground::grounds(look) && beside {
        UNKNOWN
    } else {
        look
    }
}

/// Block `z` past the service area: the Earth's ground, bedrock under sea
/// level, air over it.
pub fn earth(z: i32) -> u32 {
    if z < SEA_LEVEL {
        UNKNOWN
    } else {
        VOID
    }
}
