// pxl-portable/area.rs

//! gpu-portable: where the world's data ends, the service area — the shards
//! a source serves — and the ground's own rule at its edge (pxl-spec
//! block/limit.rs): past it, the Earth's ground, solid to sea level; inside
//! it, a leaf column with ground beside one past the area is bedrock up to
//! its own face, so the wall's outer face stands on the area's edge at every
//! zoom. The area runs on shards' edges, which no node a shard or finer
//! straddles.

use crate::cube::Ground;
use crate::{index_at, wrap_index as wrap, Zoom, LEAF_CELL, LEAF_ZOOM, SEA_LEVEL, SHARD_ZOOM};

use crate::ground::{UNKNOWN, VOID};

/// The shards a source serves, as one index both sides search
/// (`serves`): every cell at `SHARD_ZOOM` a served shard covers, a coarser
/// shard's each, by key (`shard_key`) in ascending order. On the CPU its
/// declaration's, on a GPU a buffer of it.
pub trait Area {
    /// Whether the area ends at all: before a source is declared, nothing
    /// is past it.
    fn ends(&self) -> bool;
    /// How many keys the index holds; past its served cells' it may hold
    /// `NO_KEY`s, which sort last and name none.
    fn keys(&self) -> u32;
    fn key(&self, i: u32) -> u32;
}

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

/// No cell's key: past every one, so it sorts last.
pub const NO_KEY: u32 = u32::MAX;

/// Whether the cell `c` at `SHARD_ZOOM` is served: its key found in the
/// index by halving it, a look a step.
pub fn serves(area: &impl Area, c: [i32; 2]) -> bool {
    let k = shard_key(c);
    let (mut lo, mut hi) = (0, area.keys());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if area.key(mid) < k {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    k != NO_KEY && lo < area.keys() && area.key(lo) == k
}

/// Whether the leaf tile `leaf` is past the area: it ends, and no served
/// shard is over the tile.
pub fn past(area: &impl Area, leaf: [i32; 2]) -> bool {
    let s = LEAF_ZOOM - SHARD_ZOOM;
    area.ends() && !serves(area, [leaf[0] >> s, leaf[1] >> s])
}

/// What a descent establishes of the service area at a node a shard or
/// finer, once, where it first asks one in a shard (`at`), and hands down:
/// no node a shard or finer straddles the area's edge, so it holds for every
/// node in the shard. Whether the shard is past the area, and whether it and
/// the eight shards about it are all served, `inside`: then no node in it
/// is past, nor walled (`walled`), nor has a leaf cell gone beside it
/// (`edge`), which a leaf cell's reach out of the shard ends in the eight.
/// `UNKNOWN` coarser than a shard, or before it is asked.
/// One word: known, then which of the nine shards are served, bit `(dy +
/// 1)·3 + (dx + 1)` for the shard `dx, dy` from its own; past where its own
/// is not, `inside` where all nine are, and a leaf tile in any of them past
/// or not without asking the area again (`past_leaf`).
#[derive(Clone, Copy)]
pub struct Shard(u32);

const SHARD_KNOWN: u32 = 1;
const SHARD_SERVED_SHIFT: u32 = 1;
/// The bits of which of the nine are served (`Shard::served`).
pub const SHARD_SERVED_BITS: u32 = 9;
const SHARD_ALL: u32 = (1 << SHARD_SERVED_BITS) - 1;
const _: () = assert!(LEAF_ZOOM >= SHARD_ZOOM);

impl Shard {
    pub const UNKNOWN: Shard = Shard(0);
    /// Known, every one of its nine served: nothing about it is past.
    pub const INSIDE: Shard = Shard::of_served(SHARD_ALL);

    /// The node `(zoom, c)`'s, as `self` knows it, else found: its shard and
    /// the eight about it each looked up in the area's index (`serves`).
    /// Unknown coarser than a shard.
    pub fn at(self, area: &impl Area, (zoom, c): (Zoom, [i32; 2])) -> Shard {
        if self.0 & SHARD_KNOWN != 0 || zoom < SHARD_ZOOM {
            return self;
        }
        if !area.ends() {
            return Shard(SHARD_KNOWN | SHARD_ALL << SHARD_SERVED_SHIFT);
        }
        let s = zoom - SHARD_ZOOM;
        let own = [c[0] >> s, c[1] >> s];
        // Bit `(dy + 1)·3 + (dx + 1)` for the shard `dx, dy` from its own.
        let mut served = 0u32;
        let mut k = 0u32;
        while k < 9 {
            let n = [wrap(SHARD_ZOOM, own[0] + (k % 3) as i32 - 1), own[1] + (k / 3) as i32 - 1];
            served |= (serves(area, n) as u32) << k;
            k += 1;
        }
        Shard(SHARD_KNOWN | served << SHARD_SERVED_SHIFT)
    }
    const fn known(self) -> bool {
        self.0 & SHARD_KNOWN != 0
    }
    /// Which of the nine shards about it are served, bit `(dy + 1)·3 +
    /// (dx + 1)`: what a word with no room for its known bit holds where it
    /// is known by its zoom (pxl-portable `ray`'s stop).
    pub const fn served(self) -> u32 {
        self.0 >> SHARD_SERVED_SHIFT
    }

    /// Known, the nine `served` (`served`).
    pub const fn of_served(served: u32) -> Shard {
        Shard(SHARD_KNOWN | (served & SHARD_ALL) << SHARD_SERVED_SHIFT)
    }
    /// Past the area: known so.
    pub const fn past(self) -> bool {
        self.known() && self.served() & 1 << 4 == 0
    }
    /// Known to lie, a leaf cell about it too, inside the area.
    pub const fn inside(self) -> bool {
        self.served() == SHARD_ALL
    }
    /// Whether the leaf tile `leaf` is past the area (`past`): off the nine
    /// bits of what `self`, the node `(zoom, c)`'s, knows, never the area's
    /// index. Asked only where `self` is known and the leaf lies in the
    /// node's shard or the eight about it, which every asker's shape gives:
    /// a node no wider than a shard, and a leaf a cell or a node beside it.
    /// The area is searched once a shard, where it is first known (`at`).
    pub fn past_leaf(self, (zoom, c): (Zoom, [i32; 2]), leaf: [i32; 2]) -> bool {
        let (s, t) = (zoom - SHARD_ZOOM, LEAF_ZOOM - SHARD_ZOOM);
        let d = [wrap(SHARD_ZOOM, (leaf[0] >> t) - (c[0] >> s)), (leaf[1] >> t) - (c[1] >> s)];
        debug_assert!(self.known() && d[0].abs() <= 1 && d[1].abs() <= 1, "a leaf in the node's shard's nine");
        self.served() >> ((d[1] + 1) * 3 + d[0] + 1) & 1 == 0
    }
    pub const fn word(self) -> u32 {
        self.0
    }
    pub const fn of_word(w: u32) -> Shard {
        Shard(w)
    }
}

/// The leaf tile holding the node's middle, X named as the world names it.
pub fn tile_at(zoom: Zoom, c: [i32; 2]) -> [i32; 2] {
    if zoom < LEAF_ZOOM {
        let s = LEAF_ZOOM - zoom;
        [wrap(LEAF_ZOOM, (c[0] << s) + (1 << (s - 1))), (c[1] << s) + (1 << (s - 1))]
    } else {
        let s = zoom - LEAF_ZOOM;
        [wrap(LEAF_ZOOM, c[0] >> s), c[1] >> s]
    }
}

/// Whether the service area's wall may stand in the node `(zoom, c)` asked
/// at `cell`: only a leaf's cells or finer are walled, a leaf cell with
/// ground beside one past the area. A node wider than a shard may hold the
/// area's edge; one no wider in a shard known `inside` cannot, and else is
/// asked at its corners a leaf cell out.
pub fn walled(shard: Shard, cell: Zoom, (zoom, c): (Zoom, [i32; 2])) -> bool {
    if cell < LEAF_CELL {
        return false;
    }
    if zoom < SHARD_ZOOM {
        return true;
    }
    if shard.inside() {
        return false;
    }
    let leaf = |v: i32| index_at(zoom, v, LEAF_CELL);
    let lo = [leaf(c[0]) - 1, leaf(c[1]) - 1];
    let hi = [leaf(c[0] + 1) + 1, leaf(c[1] + 1) + 1];
    let past = |l: [i32; 2]| shard.past_leaf((zoom, c), tile_at(LEAF_CELL, l));
    past(lo) || past([hi[0], lo[1]]) || past([lo[0], hi[1]]) || past(hi)
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

/// A cell's look with the area's edge laid (`edge`), `gone` where the cell
/// is past the area, `beside` where one of the eight about it is: gone,
/// void; at a leaf's cells or finer, ground beside one gone is the wall,
/// bedrock.
pub fn wall(look: u32, cell: Zoom, gone: bool, beside: bool) -> u32 {
    if gone {
        VOID
    } else if cell >= LEAF_CELL && Ground::grounds(look) && beside {
        UNKNOWN
    } else {
        look
    }
}

/// Column `b` at `cell`, its look `look`, with the area's edge laid, the
/// one rule a column is read by (pxl-portable `window::ground`) and the ways'
/// fill walls one by (`ways::Columns::walled`), on the CPU and a GPU alike:
/// gone, void, where its look is unknown; at a leaf's cells or finer, ground
/// beside a gone leaf cell is the wall (`wall`), the leaf cells the eight
/// about it lie in (`ring`) asked each once, none past the first gone.
///
/// Gone is past, one fact two ways: a cell whose look is `UNKNOWN`
/// (nothing known) is exactly one past the area. The bake writes `unknown`
/// only for tiles its coverage does not meet (pxl-geo-bake
/// terrain/absent.rs `Absence::at`), its coverage is the served shards, and
/// no window's cell is coarser than a shard, so none straddles the area's
/// edge. So the leaf cells about a column are never read to know them gone:
/// each is past or not off `shard`, the shard of `owner`, by its nine bits
/// (`Shard::past_leaf`), none past in a shard known `inside`; a column's own
/// look, read anyway, answers its own. They are asked only at a leaf's cells
/// or finer, where they lie a leaf cell past the column, within its shard's
/// nine wherever `owner`'s shard is the column's or one beside it: a block
/// a read is made for and the columns about it, or a window's square there,
/// a node far narrower than a shard.
pub fn edge(look: u32, cell: Zoom, b: [i32; 2], shard: Shard, owner: (Zoom, [i32; 2])) -> u32 {
    let gone = look == UNKNOWN;
    let beside = !gone && cell >= LEAF_CELL && Ground::grounds(look) && {
        let (r, s) = (ring(cell, b), cell - LEAF_CELL);
        let mut any = false;
        let mut k = 0u32;
        while k < 9 {
            let l = [wrap(LEAF_CELL, (b[0] >> s) + (k % 3) as i32 - 1), (b[1] >> s) + (k / 3) as i32 - 1];
            any = any || (r >> k & 1 == 1 && !shard.inside() && shard.past_leaf(owner, tile_at(LEAF_CELL, l)));
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
