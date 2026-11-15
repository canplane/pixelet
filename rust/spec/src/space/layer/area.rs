// pxl-spec/space/layer/area.rs

//! wgsl-port: where the world's data ends, and the ground's own rule at that edge
//! (pxl-content strata.rs). The service area is the set of shards a source serves. Past it lies
//! the Earth's ground, as the content gives it (pxl-engine `Content::earth`). Inside it, a leaf
//! column beside a column past the area is the wall (`is_wall`): bedrock up to the column's own
//! face, so the wall's outer face stands on the area's edge at every zoom. The area's edge runs
//! along shards' edges, which no node of a shard's size or finer straddles.

use crate::space::coord::world::wrap_idx as wrap;
use crate::space::coord::zoom::{LEAF_CELL_ZOOM, LEAF_ZOOM, SHARD_ZOOM, Zoom};

/// The shards a source serves, as one level of the pyramid at `SHARD_ZOOM` that the CPU and
/// the GPU both read (`nine_at`). Every cell at that zoom has a key (`shard_key`, one of
/// `SHARD_CELL_CNT`) and holds which of the nine cells around it, itself among them, are served
/// (`nine`); a cell beside no served one holds none. On the CPU the area is the source's
/// declaration; on a GPU it is a buffer of it.
pub trait Area {
    /// Whether the area ends at all: before a source is declared, nothing
    /// is past it.
    fn ends(&self) -> bool;
    /// The nine bits of the cell whose key is `key`, which is never `NO_KEY`.
    fn nine(&self, key: u32) -> u32;
}

/// The cells at `SHARD_ZOOM`, each with a key (`shard_key`).
pub const SHARD_CELL_CNT: u32 = 1 << (2 * SHARD_ZOOM);

/// The key of a cell at `SHARD_ZOOM`: its quadkey, the bits of its X and Y interleaved from
/// the top as a tile pyramid orders its tiles, so the cells around one lie near it in the
/// index. A row off the world has `NO_KEY`. X must be named as the world names it (`wrap`).
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

/// The shard over the node `(zoom, c)` on the ground's two axes: its cell at `SHARD_ZOOM`
/// where the node is a shard or finer, or else the node itself, since no single shard is over
/// a coarser node. X is kept as the node names it.
pub const fn shard_over(zoom: Zoom, c: [i32; 2]) -> (Zoom, [i32; 2]) {
    if zoom <= SHARD_ZOOM {
        return (zoom, c);
    }
    let s = zoom - SHARD_ZOOM;
    (SHARD_ZOOM, [c[0] >> s, c[1] >> s])
}

/// Which of the nine cells at `SHARD_ZOOM` around `own`, `own` among them, are served, as
/// `served` says of each: bit `(dy + 1)·3 + (dx + 1)` for the cell `dx, dy` away, in the order
/// `Shard::served_bits` holds them. This is a cell's word in the area (`Area::nine`), laid
/// once where the area is made (pxl-core sys/io/source.rs), so a node's shard is one read
/// (`Shard::at`).
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

/// The nine bits of the cell `c` at `SHARD_ZOOM`, read where its key addresses it; none for
/// a row off the world.
pub fn nine_at(area: &impl Area, c: [i32; 2]) -> u32 {
    let k = shard_key(c);
    if k == NO_KEY { 0 } else { area.nine(k) }
}

/// Whether the leaf tile `leaf` is past the area: the area ends, and no served shard is over
/// the tile.
pub fn past(area: &impl Area, leaf: [i32; 2]) -> bool {
    let s = LEAF_ZOOM - SHARD_ZOOM;
    area.ends() && nine_at(area, [leaf[0] >> s, leaf[1] >> s]) & OWN == 0
}

/// What a descent learns about the service area at a node of a shard's size or finer. It
/// learns it once, where it first asks a node in a shard (`at`), and hands it down: no node
/// of a shard's size or finer straddles the area's edge, so what it learns holds for every
/// node in the shard. It says whether the shard is past the area, and whether the shard and
/// the eight around it are all served (`inside`). Where they are, no node in the shard is
/// past the area or in the wall (`is_wall`), because the wall reaches one leaf cell beyond
/// the shard at most, which ends within the eight. It is `UNKNOWN` coarser than a shard, or
/// before it is asked.
///
/// It is one word: a known bit, then which of the nine shards are served, bit
/// `(dy + 1)·3 + (dx + 1)` for the shard `dx, dy` from its own. The shard is past where its
/// own bit is clear and `inside` where all nine are set, and from the bits the wall among the
/// leaf cells of the nine is known without asking the area again (`walls`, `is_wall`).
#[derive(Clone, Copy)]
pub struct Shard(u32);

pub const SHARD_KNOWN: u32 = 1;
pub const SHARD_SERVED_SHIFT: u32 = 1;
/// The bits that say which of the nine are served (`Shard::served_bits`).
const SHARD_SERVED_BITS: u32 = 9;
const SHARD_ALL: u32 = (1 << SHARD_SERVED_BITS) - 1;
/// The shard's own bit among the nine, the center.
pub const OWN: u32 = 1 << 4;
const _: () = assert!(LEAF_ZOOM >= SHARD_ZOOM);

impl Shard {
    pub const UNKNOWN: Shard = Shard(0);
    /// Known, with all nine served: nothing around it is past the area.
    pub const INSIDE: Shard = Shard(SHARD_KNOWN | SHARD_ALL << SHARD_SERVED_SHIFT);

    /// The shard of the node `(zoom, c)`: `self` where it is known, or else found from the
    /// shard's entry in the area's index (`nine_at`). It stays unknown coarser than a shard.
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
    /// Which of the nine shards around it are served, bit `(dy + 1)·3 + (dx + 1)`. A word with
    /// no room for the known bit holds these alone, where the zoom says the shard is known
    /// (pxl-engine ray.rs's stop).
    pub const fn served_bits(self) -> u32 {
        self.0 >> SHARD_SERVED_SHIFT
    }

    /// Whether the shard is known to be past the area.
    pub const fn past(self) -> bool {
        self.is_known() && self.served_bits() & OWN == 0
    }
    /// Whether the shard, and every leaf cell around it, are known to lie inside the area.
    pub const fn inside(self) -> bool {
        self.served_bits() == SHARD_ALL
    }
    /// Whether any leaf column among the leaf cells `lo` to `hi`, both included, is the wall
    /// (`is_wall`), read from the nine bits `self` knows for the node `(zoom, c)`. None is in a
    /// shard known `inside`. Ask it only where those cells, and the leaf cells around them, lie in
    /// the nine around the node's shard. Every asker's do, because the node is no wider than a
    /// shard and the leaf cells lie in it.
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
    pub const fn of_word(w: u32) -> Shard {
        Shard(w)
    }
}

/// The service area's wall, the one rule that every reader asks, on the CPU and on a GPU
/// alike. The column `b` at `cell_zoom` is the wall exactly where it is a leaf column or
/// finer and the leaf cell holding it has a leaf cell beside it past the area. So the wall is
/// one leaf column thick at every zoom past the leaves, bedrock (`Sentinel::Unknown`) up to its own
/// face, with its outer face on the area's edge. The area alone decides it, through `shard`,
/// the shard of the node `(cell_zoom, b)` (`Shard::at`). A column is read by this rule
/// (pxl-engine `clipmap::ground`), the ways' fill writes no way into the wall
/// (`way::ColumnGrid::is_walled`), and `World::within` and `World::editable` refuse it (pxl-core
/// domain/query/limit.rs).
pub fn is_wall(shard: Shard, cell_zoom: Zoom, b: [i32; 2]) -> bool {
    if cell_zoom < LEAF_CELL_ZOOM {
        return false;
    }
    let s = cell_zoom - LEAF_CELL_ZOOM;
    let l = [b[0] >> s, b[1] >> s];
    shard.walls((cell_zoom, b), [l, l])
}

/// Whether any leaf column among the leaf cells `lo` to `hi`, both included, is the wall
/// (`is_wall`), asking `area` itself a shard at a time, so the box may be of any width.
pub fn walls_in(area: &impl Area, lo_hi: [[i32; 2]; 2]) -> bool {
    area.ends() && past_about(lo_hi, |q| nine_at(area, q) & OWN == 0)
}

/// Whether a leaf cell among `lo` to `hi`, both included, or a leaf cell beside one, lies in
/// a shard that `past` says is past the area. X may run on across the world's seam. `past` is
/// asked once for each shard's cell at `SHARD_ZOOM`, with X as the world names it. A shard is
/// many leaf cells a side, so asking each shard with the eight around it covers every leaf
/// cell's neighbors: this is the wall's rule (`is_wall`) over every leaf column in the box.
fn past_about([lo, hi]: [[i32; 2]; 2], past: impl Fn([i32; 2]) -> bool) -> bool {
    let t = LEAF_CELL_ZOOM - SHARD_ZOOM;
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
const _: () = assert!(LEAF_CELL_ZOOM > SHARD_ZOOM);
