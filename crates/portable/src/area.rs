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

/// What a descent establishes of the service area at a node a shard or
/// finer, once, where it first asks one in a shard (`at`), and hands down:
/// no node a shard or finer straddles the area's edge, so it holds for every
/// node in the shard. Whether the shard is past the area, and whether it and
/// the eight shards about it are all served, `inside`: then no node in it
/// is past, nor walled (`walled`), nor has a leaf cell gone beside it
/// (`edge`), which a leaf cell's reach out of the shard ends in the eight.
/// `UNKNOWN` coarser than a shard, or before it is asked.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Shard(u32);

const SHARD_KNOWN: u32 = 1;
const SHARD_PAST: u32 = 2;
const SHARD_INSIDE: u32 = 4;

impl Shard {
    pub const UNKNOWN: Shard = Shard(0);

    /// The node `(zoom, c)`'s, as `self` knows it, else found: one pass over
    /// the served shards for its shard and the eight about it. Unknown
    /// coarser than a shard.
    pub fn at(self, area: &impl Area, (zoom, c): (Zoom, [i32; 2])) -> Shard {
        if self.0 & SHARD_KNOWN != 0 || zoom < SHARD_ZOOM {
            return self;
        }
        if !area.ends() {
            return Shard(SHARD_KNOWN | SHARD_INSIDE);
        }
        let s = zoom - SHARD_ZOOM;
        let own = [c[0] >> s, c[1] >> s];
        // Bit `(dy + 1)·3 + (dx + 1)` for the shard `dx, dy` from its own.
        let mut served = 0u32;
        let mut i = 0;
        while i < area.shards() {
            let (z, at) = area.shard(i);
            if z <= SHARD_ZOOM {
                let d = SHARD_ZOOM - z;
                let mut k = 0u32;
                while k < 9 {
                    let n = [wrap(SHARD_ZOOM, own[0] + (k % 3) as i32 - 1), own[1] + (k / 3) as i32 - 1];
                    if n[0] >> d == at[0] && n[1] >> d == at[1] {
                        served |= 1 << k;
                    }
                    k += 1;
                }
            }
            i += 1;
        }
        let past = if served & 1 << 4 == 0 { SHARD_PAST } else { 0 };
        let inside = if served == 0x1ff { SHARD_INSIDE } else { 0 };
        Shard(SHARD_KNOWN | past | inside)
    }
    /// Past the area: known so.
    pub const fn past(self) -> bool {
        self.0 & SHARD_PAST != 0
    }
    /// Known to lie, a leaf cell about it too, inside the area.
    pub const fn inside(self) -> bool {
        self.0 & SHARD_INSIDE != 0
    }
    pub const fn word(self) -> u32 {
        self.0
    }
    pub const fn of_word(w: u32) -> Shard {
        Shard(w)
    }
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
/// area's edge; one no wider in a shard known `inside` cannot, and else is
/// asked at its corners a leaf cell out.
pub fn walled(area: &impl Area, shard: Shard, cell: Zoom, (zoom, c): (Zoom, [i32; 2])) -> bool {
    if cell < LEAF_CELL {
        return false;
    }
    if zoom < SHARD_ZOOM {
        return true;
    }
    if shard.inside() {
        return false;
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

/// A cell's look with the area's edge laid, the one rule every grid reads it
/// by (pxl-spec block/sample.rs `Grid::wall_off`, gpu-query's `column`):
/// `gone` where the cell is past the area or nothing is known of it; at a
/// leaf's cells or finer, ground beside one gone is the wall (`wall`), where
/// the leaf cells the eight about the block `b` at `cell` lie in (`ring`) are
/// asked by their offset in leaf cells (`gone_at`), each once, none past
/// the first gone.
pub fn edge(look: u32, cell: Zoom, b: [i32; 2], gone: bool, gone_at: impl Fn(i32, i32) -> bool) -> u32 {
    let beside = !gone && cell >= LEAF_CELL && Ground::grounds(look) && {
        let r = ring(cell, b);
        let mut any = false;
        let mut k = 0u32;
        while k < 9 {
            any = any || (r >> k & 1 == 1 && gone_at((k % 3) as i32 - 1, (k / 3) as i32 - 1));
            k += 1;
        }
        any
    };
    wall(look, cell, gone, beside)
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
