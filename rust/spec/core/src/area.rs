// pxl-spec-core/area.rs

//! gpu-portable: where the world's data ends, the service area — the shards
//! a source serves — and the ground's own rule at its edge (pxl-spec
//! limit.rs): past it, the Earth's ground, solid to sea level; inside
//! it, a leaf column with ground beside one past the area is bedrock up to
//! its own face, so the wall's outer face stands on the area's edge at every
//! zoom. The area runs on shards' edges, which no node a shard or finer
//! straddles.

use crate::cube::Ground;
use crate::{idx_at, wrap_idx as wrap, Zoom, LEAF_CELL, LEAF_ZOOM, SEA_LEVEL, SHARD_ZOOM};

use crate::ground::{UNKNOWN, VOID};

/// The shards a source serves, as the pyramid's own level at `SHARD_ZOOM`
/// both sides read (`found`): every cell there, addressed by its key
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

/// Which of the nine cells at `SHARD_ZOOM` about `own` are served, as
/// `served` says of each, bit `(dy + 1)·3 + (dx + 1)` for the cell `dx, dy`
/// from it, its own among them, as `Shard::served` holds them: a cell's
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
pub fn found(area: &impl Area, c: [i32; 2]) -> u32 {
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
    area.ends() && found(area, [leaf[0] >> s, leaf[1] >> s]) & OWN == 0
}

/// What a descent establishes of the service area at a node a shard or
/// finer, once, where it first asks one in a shard (`at`), and hands down:
/// no node a shard or finer straddles the area's edge, so it holds for every
/// node in the shard. Whether the shard is past the area, and whether it and
/// the eight shards about it are all served, `inside`: then no node in it
/// is past, nor walled (`is_walled`), nor has a leaf cell gone beside it
/// (`gone_ring`), which a leaf cell's reach out of the shard ends in the eight.
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
    /// nine off its entry in the area's index (`found`). Unknown coarser
    /// than a shard.
    pub fn at(self, area: &impl Area, (zoom, c): (Zoom, [i32; 2])) -> Shard {
        if self.0 & SHARD_KNOWN != 0 || zoom < SHARD_ZOOM {
            return self;
        }
        if !area.ends() {
            return Shard(SHARD_KNOWN | SHARD_ALL << SHARD_SERVED_SHIFT);
        }
        let s = zoom - SHARD_ZOOM;
        Shard(SHARD_KNOWN | found(area, [c[0] >> s, c[1] >> s]) << SHARD_SERVED_SHIFT)
    }
    const fn is_known(self) -> bool {
        self.0 & SHARD_KNOWN != 0
    }
    /// Which of the nine shards about it are served, bit `(dy + 1)·3 +
    /// (dx + 1)`: what a word with no room for its known bit holds where it
    /// is known by its zoom (pxl-spec-core `ray`'s stop).
    pub const fn served(self) -> u32 {
        self.0 >> SHARD_SERVED_SHIFT
    }

    /// Past the area: known so.
    pub const fn past(self) -> bool {
        self.is_known() && self.served() & OWN == 0
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
        let d = [
            wrap(SHARD_ZOOM, (leaf[0] >> t) - (c[0] >> s)),
            (leaf[1] >> t) - (c[1] >> s),
        ];
        debug_assert!(
            self.is_known() && d[0].abs() <= 1 && d[1].abs() <= 1,
            "a leaf in the node's shard's nine"
        );
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
        [
            wrap(LEAF_ZOOM, (c[0] << s) + (1 << (s - 1))),
            (c[1] << s) + (1 << (s - 1)),
        ]
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
pub fn is_walled(shard: Shard, cell: Zoom, (zoom, c): (Zoom, [i32; 2])) -> bool {
    if cell < LEAF_CELL {
        return false;
    }
    if zoom < SHARD_ZOOM {
        return true;
    }
    if shard.inside() {
        return false;
    }
    let leaf = |v: i32| idx_at(zoom, v, LEAF_CELL);
    let lo = [leaf(c[0]) - 1, leaf(c[1]) - 1];
    let hi = [leaf(c[0] + 1) + 1, leaf(c[1] + 1) + 1];
    let past = |l: [i32; 2]| shard.past_leaf((zoom, c), tile_at(LEAF_CELL, l));
    past(lo) || past([hi[0], lo[1]]) || past([lo[0], hi[1]]) || past(hi)
}

/// The eight leaf cells about one, the order a mask of them is in
/// (`ring`, `gone_ring`): bit `k` the cell `beside(k)` from its own, row by
/// row, its own never.
pub const fn beside(k: u32) -> [i32; 2] {
    let j = if k < 4 { k } else { k + 1 };
    [(j % 3) as i32 - 1, (j / 3) as i32 - 1]
}

/// The leaf cells about the leaf cell holding the column `b` at `cell`, a
/// leaf's or finer, that its eight neighbors lie in (`beside`'s bits). A
/// column within its leaf cell touches none; one on a leaf's edge or corner,
/// the cells across them; at the leaves, all eight. Whether the column has
/// ground beside one gone (`laid`) is whether any of these is.
pub fn ring(cell: Zoom, b: [i32; 2]) -> u32 {
    let m = (1 << (cell - LEAF_CELL)) - 1;
    // Per axis, the leaf offsets its neighbors reach: 0 always, -1 at the
    // leaf cell's low edge, +1 at its high one.
    let reach = |v: i32| 0b010 | ((v & m == 0) as u32) | ((v & m == m) as u32) << 2;
    let (x, y) = (reach(b[0]), reach(b[1]));
    // Bit `k` where it reaches `beside(k)` along both axes.
    let mut r = 0;
    let mut k = 0;
    while k < 8 {
        let d = beside(k);
        r |= ((x >> (d[0] + 1)) & (y >> (d[1] + 1)) & 1) << k;
        k += 1;
    }
    r
}

/// Which of the eight leaf cells about the leaf cell `l` are past the area
/// (`beside`'s bits), off the nine bits of `shard`, the shard of `node`,
/// none past in a shard known `inside`: what the fill lays a cell's edge by
/// once (pxl-spec-core `fill::heights`), for every read of it after.
///
/// Gone is past, one fact two ways: a cell whose look is `UNKNOWN`
/// (nothing known) is exactly one past the area. The bake writes `unknown`
/// only for tiles its coverage does not meet (pxl-geo-bake
/// terrain/absent.rs `Absence::at`), its coverage is the served shards, and
/// no window's cell is coarser than a shard, so none straddles the area's
/// edge. So the leaf cells about one are never read to know them gone:
/// each is past or not off the shard's bits (`Shard::past_leaf`), and a
/// cell's own look answers its own. They lie a leaf cell past `l`, inside
/// its shard's nine wherever `node`'s shard is `l`'s, as a leaf window's
/// brick is, a node far narrower than a shard.
pub fn gone_ring(shard: Shard, node: (Zoom, [i32; 2]), l: [i32; 2]) -> u32 {
    if shard.inside() {
        return 0;
    }
    let mut gone = 0;
    let mut k = 0;
    while k < 8 {
        let d = beside(k);
        gone |= (shard.past_leaf(
            node,
            tile_at(LEAF_CELL, [wrap(LEAF_CELL, l[0] + d[0]), l[1] + d[1]]),
        ) as u32)
            << k;
        k += 1;
    }
    gone
}

/// The column `b` at `cell` of a cell laid as the fill lays it (`laid` low
/// byte its own look, high the leaf cells about it gone, `gone_ring`), with
/// the area's edge laid, the one rule a column is read by (pxl-spec-core
/// `window::ground`) and the ways' fill walls one by
/// (`way::Columns::is_walled`), on the CPU and a GPU alike: at a leaf's cells
/// or finer, ground beside a gone leaf cell, of those the column touches
/// (`ring`), is the wall, bedrock (`UNKNOWN`) up to its own face. A gone
/// cell is bedrock already, its look `UNKNOWN` (gone is past), and its top
/// the Earth's at sea level as its fill lays it (pxl-spec-core
/// `fill::heights`): the wall at sea level, so a cell of it is the Earth's
/// (`earth`). A column read off a brick coarser than a leaf's cells, a
/// stand-in, has none gone about it, so no wall: the stand-in is the world
/// at its zoom, and the wall is a fact of the leaves it does not hold.
pub fn laid(laid: u32, cell: Zoom, b: [i32; 2]) -> u32 {
    let look = laid & 0xff;
    if cell >= LEAF_CELL && Ground::grounds(look) && ring(cell, b) & laid >> 8 != 0 {
        UNKNOWN
    } else {
        look
    }
}

/// Cell `z` past the service area: the Earth's ground, bedrock under sea
/// level, air over it.
pub fn earth(z: i32) -> u32 {
    if z < SEA_LEVEL {
        UNKNOWN
    } else {
        VOID
    }
}
