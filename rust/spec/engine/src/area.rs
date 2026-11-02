// pxl-engine/area.rs

//! gpu-portable: where the world's data ends, the service area — the shards
//! a source serves — and the ground's own rule at its edge (pxl-spec
//! limit.rs): past it, the Earth's ground, solid to sea level; inside
//! it, a leaf column beside one past the area is the wall (`is_wall`),
//! bedrock up to its own face, so the wall's outer face stands on the area's
//! edge at every zoom. The area runs on shards' edges, which no node a
//! shard or finer straddles.

use crate::{idx_at, wrap_idx as wrap, Zoom, LEAF_CELL, LEAF_ZOOM, SEA_LEVEL, SHARD_ZOOM};

use crate::ground::{UNKNOWN, VOID};

/// The shards a source serves, as the pyramid's own level at `SHARD_ZOOM`
/// both sides read (`nine_at`): every cell there, addressed by its key
/// (`shard_key`, `SHARD_CELLS` of them), holding which of the nine about it
/// are served (`nine`), none where it is beside no served one. On the CPU
/// its declaration's, on a GPU a buffer of it.
pub trait Area {
    /// Whether the area ends at all: before a source is declared, nothing
    /// is past it.
    fn ends(&self) -> bool;
    /// The nine of the cell whose key is `key`, never `NO_KEY`.
    fn nine(&self, key: u32) -> u32;
}

/// The cells at `SHARD_ZOOM`, a key each (`shard_key`).
pub const SHARD_CELLS: u32 = 1 << (2 * SHARD_ZOOM);

/// A cell at `SHARD_ZOOM`'s key: its quadkey, its X's and Y's bits
/// interleaved from the top, as a tile pyramid orders its tiles, so the
/// cells about one lie near it in the index; `NO_KEY` for a row off the
/// world. X is named as the world names it (`wrap`).
pub const fn shard_key(c: [i32; 2]) -> u32 {
    if c[1] < 0 || c[1] >= 1 << SHARD_ZOOM {
        return NO_KEY;
    }
    let (mut k, mut b) = (0, SHARD_ZOOM);
    while b > 0 {
        b -= 1;
        k = k << 2 | (c[0] as u32 >> b & 1) | (c[1] as u32 >> b & 1) << 1;
    }
    k
}

/// No cell's key: a row off the world.
pub const NO_KEY: u32 = u32::MAX;

/// The shard over the node `(zoom, c)` of the ground's two axes: its cell at
/// `SHARD_ZOOM` where the node is a shard or finer, else the node itself,
/// which no one shard is over. X as the node names it.
pub const fn shard_over(zoom: Zoom, c: [i32; 2]) -> (Zoom, [i32; 2]) {
    if zoom <= SHARD_ZOOM {
        return (zoom, c);
    }
    let s = zoom - SHARD_ZOOM;
    (SHARD_ZOOM, [c[0] >> s, c[1] >> s])
}

/// Which of the nine cells at `SHARD_ZOOM` about `own` are served, as
/// `served` says of each, bit `(dy + 1)·3 + (dx + 1)` for the cell `dx, dy`
/// from it, its own among them, as `Shard::served_bits` holds them: a cell's
/// word in the area (`Area::nine`), laid once where the area is made
/// (pxl-kernel sys/io/source.rs), so a node's shard is one read (`Shard::at`).
pub fn nine(served: impl Fn([i32; 2]) -> bool, own: [i32; 2]) -> u32 {
    let mut bits = 0u32;
    let mut k = 0u32;
    while k < 9 {
        let n = [
            wrap(SHARD_ZOOM, own[0] + (k % 3) as i32 - 1),
            own[1] + (k / 3) as i32 - 1,
        ];
        bits |= (served(n) as u32) << k;
        k += 1;
    }
    bits
}

/// The cell `c` at `SHARD_ZOOM`'s nine, read where its key addresses it;
/// none for a row off the world.
pub fn nine_at(area: &impl Area, c: [i32; 2]) -> u32 {
    let k = shard_key(c);
    if k == NO_KEY {
        0
    } else {
        area.nine(k)
    }
}

/// Whether the leaf tile `leaf` is past the area: it ends, and no served
/// shard is over the tile.
pub fn past(area: &impl Area, leaf: [i32; 2]) -> bool {
    let s = LEAF_ZOOM - SHARD_ZOOM;
    area.ends() && nine_at(area, [leaf[0] >> s, leaf[1] >> s]) & OWN == 0
}

/// What a descent establishes of the service area at a node a shard or
/// finer, once, where it first asks one in a shard (`at`), and hands down:
/// no node a shard or finer straddles the area's edge, so it holds for every
/// node in the shard. Whether the shard is past the area, and whether it and
/// the eight shards about it are all served, `inside`: then no node in it
/// is past, nor the wall (`is_wall`), whose reach a leaf cell out of the
/// shard ends in the eight.
/// `UNKNOWN` coarser than a shard, or before it is asked.
/// One word: known, then which of the nine shards are served, bit `(dy +
/// 1)·3 + (dx + 1)` for the shard `dx, dy` from its own; past where its own
/// is not, `inside` where all nine are, and a leaf tile in any of them past
/// or not without asking the area again (`past_leaf`).
#[derive(Clone, Copy)]
pub struct Shard(u32);

const SHARD_KNOWN: u32 = 1;
const SHARD_SERVED_SHIFT: u32 = 1;
/// The bits of which of the nine are served (`Shard::served_bits`).
const SHARD_SERVED_BITS: u32 = 9;
const SHARD_ALL: u32 = (1 << SHARD_SERVED_BITS) - 1;
/// Its own among the nine, the center.
const OWN: u32 = 1 << 4;
const _: () = assert!(LEAF_ZOOM >= SHARD_ZOOM);

impl Shard {
    pub const UNKNOWN: Shard = Shard(0);
    /// Known, every one of its nine served: nothing about it is past.
    pub const INSIDE: Shard = Shard(SHARD_KNOWN | SHARD_ALL << SHARD_SERVED_SHIFT);

    /// The node `(zoom, c)`'s, as `self` knows it, else found: its shard's
    /// nine off its entry in the area's index (`nine_at`). Unknown coarser
    /// than a shard.
    pub fn at(self, area: &impl Area, (zoom, c): (Zoom, [i32; 2])) -> Shard {
        if self.0 & SHARD_KNOWN != 0 || zoom < SHARD_ZOOM {
            return self;
        }
        if !area.ends() {
            return Shard(SHARD_KNOWN | SHARD_ALL << SHARD_SERVED_SHIFT);
        }
        let s = zoom - SHARD_ZOOM;
        Shard(SHARD_KNOWN | nine_at(area, [c[0] >> s, c[1] >> s]) << SHARD_SERVED_SHIFT)
    }
    const fn is_known(self) -> bool {
        self.0 & SHARD_KNOWN != 0
    }
    /// Which of the nine shards about it are served, bit `(dy + 1)·3 +
    /// (dx + 1)`: what a word with no room for its known bit holds where it
    /// is known by its zoom (pxl-engine `ray`'s stop).
    pub const fn served_bits(self) -> u32 {
        self.0 >> SHARD_SERVED_SHIFT
    }

    /// Past the area: known so.
    pub const fn past(self) -> bool {
        self.is_known() && self.served_bits() & OWN == 0
    }
    /// Known to lie, a leaf cell about it too, inside the area.
    pub const fn inside(self) -> bool {
        self.served_bits() == SHARD_ALL
    }
    /// Whether the leaf tile `leaf` is past the area (`past`): off the nine
    /// bits of what `self`, the node `(zoom, c)`'s, knows, never the area's
    /// index. Asked only where `self` is known and the leaf lies in the
    /// node's shard or the eight about it, which every asker's shape gives:
    /// a node no wider than a shard, and a leaf a cell or a node beside it.
    /// The area is searched once a shard, where it is first known (`at`).
    pub fn past_leaf(self, (zoom, c): (Zoom, [i32; 2]), leaf: [i32; 2]) -> bool {
        let (s, t) = (zoom - SHARD_ZOOM, LEAF_ZOOM - SHARD_ZOOM);
        let d = [
            wrap(SHARD_ZOOM, (leaf[0] >> t) - (c[0] >> s)),
            (leaf[1] >> t) - (c[1] >> s),
        ];
        debug_assert!(
            self.is_known() && d[0].abs() <= 1 && d[1].abs() <= 1,
            "a leaf in the node's shard's nine"
        );
        self.served_bits() >> ((d[1] + 1) * 3 + d[0] + 1) & 1 == 0
    }
    /// Whether a leaf column among the leaf cells `lo` to `hi`, both held,
    /// is the wall (`is_wall`), off the nine bits of what `self`, the node
    /// `(zoom, c)`'s, knows: none in a shard known `inside`. Asked only where
    /// they and the leaf cells about them lie in the node's shard's nine,
    /// as every asker's are, the node no wider than a shard and the leaf
    /// cells in it.
    pub fn walls(self, (zoom, c): (Zoom, [i32; 2]), lo_hi: [[i32; 2]; 2]) -> bool {
        if self.inside() {
            return false;
        }
        let s = zoom - SHARD_ZOOM;
        let own = [c[0] >> s, c[1] >> s];
        past_about(lo_hi, |q| {
            let d = [wrap(SHARD_ZOOM, q[0] - own[0]), q[1] - own[1]];
            debug_assert!(
                self.is_known() && d[0].abs() <= 1 && d[1].abs() <= 1,
                "a leaf cell in the node's shard's nine"
            );
            self.served_bits() >> ((d[1] + 1) * 3 + d[0] + 1) & 1 == 0
        })
    }
    pub const fn word(self) -> u32 {
        self.0
    }
    pub(crate) const fn of_word(w: u32) -> Shard {
        Shard(w)
    }
}

/// The leaf tile holding the node's middle, X named as the world names it.
pub fn tile_at(zoom: Zoom, c: [i32; 2]) -> [i32; 2] {
    if zoom < LEAF_ZOOM {
        let s = LEAF_ZOOM - zoom;
        [
            wrap(LEAF_ZOOM, (c[0] << s) + (1 << (s - 1))),
            (c[1] << s) + (1 << (s - 1)),
        ]
    } else {
        let s = zoom - LEAF_ZOOM;
        [wrap(LEAF_ZOOM, c[0] >> s), c[1] >> s]
    }
}

/// The service area's wall, its one rule, which every reader asks on the
/// CPU and a GPU alike: the column `b` at `cell` is the wall exactly where it
/// is a leaf's or finer and the leaf cell holding it has a leaf cell beside
/// it past the area, so the wall is a leaf column thick at every zoom past
/// the leaves, bedrock (`UNKNOWN`) up to its own face, its outer face on the
/// area's edge. The area alone says it, off `shard`, the node `(cell, b)`'s
/// (`Shard::at`): never the brick the column is read off, nor none, nor its
/// palette index. What a column is read as (`clipmap::ground`), what the
/// ways' fill writes no way in (`way::Columns::is_walled`), and what
/// `World::within` and `World::editable` refuse (pxl-kernel
/// domain/query/limit.rs).
pub fn is_wall(shard: Shard, cell: Zoom, b: [i32; 2]) -> bool {
    if cell < LEAF_CELL {
        return false;
    }
    let s = cell - LEAF_CELL;
    let l = [b[0] >> s, b[1] >> s];
    shard.walls((cell, b), [l, l])
}

/// Whether the wall (`is_wall`) may stand in the node `(zoom, c)` asked at
/// `cell`: one of its columns is the wall, a leaf cell in it or beside it
/// past the area (`Shard::walls`). A node wider than a shard may hold the
/// area's edge anywhere.
pub fn is_walled(shard: Shard, cell: Zoom, (zoom, c): (Zoom, [i32; 2])) -> bool {
    if cell < LEAF_CELL {
        return false;
    }
    if zoom < SHARD_ZOOM {
        return true;
    }
    let first = |v: i32| idx_at(zoom, v, LEAF_CELL);
    let last = |v: i32| (idx_at(zoom, v + 1, LEAF_CELL) - 1).max(first(v));
    shard.walls(
        (zoom, c),
        [[first(c[0]), first(c[1])], [last(c[0]), last(c[1])]],
    )
}

/// Whether a leaf column among the leaf cells `lo` to `hi`, both held, is
/// the wall (`is_wall`), off `area` itself a shard at a time: the leaf
/// columns of a box any wide.
pub fn walls_in(area: &impl Area, lo_hi: [[i32; 2]; 2]) -> bool {
    area.ends() && past_about(lo_hi, |q| nine_at(area, q) & OWN == 0)
}

/// Whether a leaf cell among `lo` to `hi`, both held, X counted on across
/// the world's seam, or beside one lies in a shard `past` says is past the
/// area, `past` asked of each shard's cell at `SHARD_ZOOM` once, X as the
/// world names it. A leaf cell past the area has one beside it past too,
/// its shard many leaf cells a side, so its own is asked with the eight
/// about it: the wall's rule (`is_wall`) over every leaf column in the box.
fn past_about([lo, hi]: [[i32; 2]; 2], past: impl Fn([i32; 2]) -> bool) -> bool {
    let t = LEAF_CELL - SHARD_ZOOM;
    let mut y = (lo[1] - 1) >> t;
    while y <= (hi[1] + 1) >> t {
        let mut x = (lo[0] - 1) >> t;
        while x <= (hi[0] + 1) >> t {
            if past([wrap(SHARD_ZOOM, x), y]) {
                return true;
            }
            x += 1;
        }
        y += 1;
    }
    false
}
const _: () = assert!(LEAF_CELL > SHARD_ZOOM);

/// Cell `z` past the service area: the Earth's ground, bedrock under sea
/// level, air over it.
pub fn earth(z: i32) -> u32 {
    if z < SEA_LEVEL {
        UNKNOWN
    } else {
        VOID
    }
}
