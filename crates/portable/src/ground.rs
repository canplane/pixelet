// pxl-portable/ground.rs

//! gpu-portable: the ground's grid at a cell, the one rule pxl-spec
//! block/sample.rs `at` composes before what overwrites it: a cell's look and
//! height off the tile drawing it, their fold a zoom coarser, and its blocks
//! by depth with a cave's air where a cave may take them. What overwrites it
//! is not here: the ways by their own rule (`ways.rs`), and what is placed
//! by its own (`placed.rs`).
//!
//! A look and a height are stored as a byte and an i16 and read into a word
//! here, since WGSL has no narrower integer.
//!
//! Heights are f32 here, as WGSL has them. WGSL's f32 is not strictly IEEE (a
//! division's last place, a contraction into a fused multiply-add), so a
//! height a CPU and a GPU compute from this one source can differ in the
//! last bit where it rounds to a block at a half.

use crate::area::{earth, walled, Area, Shard};
use crate::cast::{In, Kind};
use crate::caves::Cave;
use crate::cube::{ground, sentinel, Ground};
pub use crate::lattice::TILE_CELLS;
use crate::lattice::{CELL_CENTER, GROUND};
use crate::window::Found;
use crate::{Zoom, LEAF_CELL, LEAF_ZOOM, SEA_LEVEL, SHARD_ZOOM, UNIT_ZOOM};

// The catalog's looks this rule names, read into a word.
const VOID: u32 = sentinel::void as u32;
const UNKNOWN: u32 = sentinel::unknown as u32;
const SAND: u32 = ground::sand as u32;
const BEACH: u32 = ground::beach as u32;
const WATER: u32 = ground::water as u32;
const SOIL: u32 = ground::soil as u32;
const SANDSTONE: u32 = ground::sandstone as u32;
const STONE: u32 = ground::stone as u32;

/// Inner elevation samples across one leaf tile: a tile is about twice its
/// side in units in meters wide and the source is about 30 m, rounded to 32 m.
pub const TILE_ELE_SPAN: u32 = 1 << (UNIT_ZOOM - LEAF_ZOOM + 1 - 5);

/// Two samples of padding give the bicubic a 4×4 kernel at every edge texel,
/// and let the ground height 32 cells past the tile be read without the
/// neighboring tile resident.
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = TILE_ELE_SPAN + 2 * ELE_PAD;

/// Sample (x, y) of a padded elevation grid, rows from the south.
pub const fn ele_index(x: u32, y: u32) -> usize {
    (y * ELE_PADDED_SPAN + x) as usize
}

/// A layer of ground cells under a face, in leaf cells, before the stone.
const FILLER_DEPTH: i32 = 3;
/// Sandstone under a sand or beach face's filler, in leaf cells.
const SANDSTONE_DEPTH: i32 = 2;
/// The bedrock's top under a face, this far under it or under sea level,
/// whichever is lower, in leaf cells (`bedrock`).
pub const BEDROCK_DEPTH: i32 = 64;

/// The bedrock's top under a face at `face` leaf cells, in leaf cells: the
/// lowest a block may reach there, every walk's, edit's and column's.
pub fn bedrock(face: i32) -> i32 {
    face.min(SEA_LEVEL) - BEDROCK_DEPTH
}

/// The height the surface draws at the center of a tile's cell `(x, y)`,
/// in leaf cells, off its padded elevation samples `ele` (`ele_index`): the
/// one producer of a cell's height, which a GPU runs once a cell of a held
/// tile (`heights_cs`), and the CPU reads back.
pub fn height(ele: impl Fn(usize) -> i32, [x, y]: [u32; 2]) -> i32 {
    bicubic(ele, x as f32 + CELL_CENTER, y as f32 + CELL_CENTER)
}

/// A tile's padded samples as a GPU holds them, 2 a word.
pub const ELE_WORDS: u32 = (ELE_PADDED_SPAN * ELE_PADDED_SPAN).div_ceil(2);

/// An entry of a bound as a word, its lowest in the low half and its
/// highest in the high: each a cell's height, an i16.
pub fn bound_word([lo, hi]: [i32; 2]) -> i32 {
    lo & 0xffff | hi << 16
}

/// `bound_word`'s lowest and highest.
pub fn bound_of(w: i32) -> [i32; 2] {
    [w << 16 >> 16, w >> 16]
}

/// A cell's word's four looks, `dx + 2·dy`.
pub fn looks(word: u32) -> [u32; 4] {
    [word & 0xff, word >> 8 & 0xff, word >> 16 & 0xff, word >> 24]
}

/// Four looks as a cell's word.
pub fn word([a, b, c, d]: [u32; 4]) -> u32 {
    a | b << 8 | c << 16 | d << 24
}

/// A cell's own look at `cell`: the majority of the four it folds, as the
/// bake folded it (`fold`).
pub fn own(w: u32, cell: Zoom) -> u32 {
    majority(looks(w), cell)
}

/// Four cells a zoom finer, `dx + 2·dy`, as one: the looks' majority, the
/// mean height of those grounded (none grounded, sea level), and the four
/// looks it folds.
pub fn fold(four: [(u32, i32); 4], cell: Zoom) -> (u32, f32, [u32; 4]) {
    let mut looks = [VOID; 4];
    let (mut sum, mut count) = (0i32, 0i32);
    let mut k = 0;
    while k < 4 {
        let (look, h) = four[k];
        looks[k] = look;
        if Ground::grounds(look) {
            sum += h;
            count += 1;
        }
        k += 1;
    }
    let h = if count > 0 { sum as f32 / count as f32 } else { SEA_LEVEL as f32 };
    (majority(looks, cell), h, looks)
}

/// A held cell as a grid reads it, off its word `w`, its height `h` and
/// `from`, the zoom the word is a cell of (a coarser window's where one
/// stands in, the leaf's past the leaves): its look, height and the four
/// looks it folds. A cell's look is decided at the zoom of the cell it is
/// made of, so a word's tie breaks at `from` (`majority`).
pub fn read_cell(w: u32, h: i32, from: Zoom) -> (u32, f32, [u32; 4]) {
    (own(w, from), h as f32, looks(w))
}

/// A cell one zoom coarser than those held, `zoom`'s, as a grid reads it: the
/// fold of its four children (`dx + 2·dy`), each read as held (`read_cell`), its look
/// decided at its own zoom. No coarser is asked: a ray's zooms start at the
/// top tile's cells (pxl-kernel view/lens.rs `asked`), and only a ground read
/// for a tile under the top reaches this one (pxl-spec block/sample.rs
/// `Grid::read`, which refuses more).
pub fn coarser(four: [(u32, i32, Zoom); 4], zoom: Zoom) -> (u32, f32, [u32; 4]) {
    let mut children = [(VOID, 0); 4];
    let mut k = 0;
    while k < 4 {
        let (w, h, from) = four[k];
        children[k] = (own(w, from), h);
        k += 1;
    }
    fold(children, zoom)
}

/// Tile cells an elevation sample spans.
const CELL_STEP: f32 = (TILE_CELLS / TILE_ELE_SPAN) as f32;

/// The first of the four samples the surface reads along an axis about a
/// texel coordinate, and how far past the second the coordinate falls; a
/// cell's center is its index plus a half.
pub fn window(t: f32) -> (i32, f32) {
    let s = t / CELL_STEP + ELE_PAD as f32 - 0.5;
    let f = libm::floorf(s);
    (f as i32 - 1, s - f)
}

/// A sample index on the padded grid: past its edge, the edge sample stands.
pub fn held(i: i32) -> u32 {
    i.clamp(0, ELE_PADDED_SPAN as i32 - 1) as u32
}

/// Catmull-Rom's four weights at `t` past the second sample.
fn catmull_rom(t: f32) -> [f32; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [-0.5 * t3 + t2 - 0.5 * t, 1.5 * t3 - 2.5 * t2 + 1.0, -1.5 * t3 + 2.0 * t2 + 0.5 * t, 0.5 * t3 - 0.5 * t2]
}

/// The drawn surface: Catmull-Rom over the 4 × 4 padded samples about a
/// texel point, in cells, held to the samples' range, so a bound over the
/// surface is the samples' own.
pub fn bicubic(ele: impl Fn(usize) -> i32, tx: f32, ty: f32) -> i32 {
    let (x0, fx) = window(tx);
    let (y0, fy) = window(ty);
    let (wx, wy) = (catmull_rom(fx), catmull_rom(fy));
    let (mut out, mut lo, mut hi) = (0.0f32, i32::MAX, i32::MIN);
    let mut j = 0;
    while j < 4 {
        let py = held(y0 + j);
        let mut row = 0.0f32;
        let mut i = 0;
        while i < 4 {
            let v = ele(ele_index(held(x0 + i), py));
            lo = lo.min(v);
            hi = hi.max(v);
            row += v as f32 * wx[i as usize];
            i += 1;
        }
        out += row * wy[j as usize];
        j += 1;
    }
    libm::rintf(out.clamp(lo as f32, hi as f32)) as i32
}

/// The value filling more than half of a 2×2 (or 2×2×2) block, `v[dx + 2·dy +
/// 4·dz]`; otherwise the tie child's. The tie corner alternates with the
/// parent cell's zoom, since a fixed corner drifts the map half a coarse cell a step.
pub fn majority<T: Copy + Eq, const N: usize>(v: [T; N], parent: Zoom) -> T {
    let tie = v[if parent & 1 == 1 { 0 } else { N - 1 }];
    let count = |x: T| {
        let (mut n, mut k) = (0, 0);
        while k < N {
            n += usize::from(v[k] == x);
            k += 1;
        }
        n
    };
    let mut half: Option<T> = None;
    let mut k = 0;
    while k < N {
        let x = v[k];
        let n = count(x);
        if 2 * n > N {
            return x;
        }
        if 2 * n == N {
            match half {
                Some(h) if h != x => return tie,
                _ => half = Some(x),
            }
        }
        k += 1;
    }
    match half {
        Some(h) => h,
        None => tie,
    }
}

/// A height in leaf cells as blocks of `cell`: shifted past the leaves, the
/// mean rounded to even under them, as WGSL's `round` does.
pub fn blocks(cell: Zoom, h: f32) -> i32 {
    let d = cell as i32 - LEAF_CELL as i32;
    if d >= 0 {
        (h as i32) << d
    } else {
        libm::rintf(h / (1 << -d) as f32) as i32
    }
}

/// Blocks of `cell` a leaf cell is tall: one where a cell folds the leaves.
pub fn leaf_blocks(cell: Zoom) -> i32 {
    1 << (cell as i32 - LEAF_CELL as i32).max(0)
}

/// The face of a top of `top` blocks of `cell`, in leaf cells, as the grid
/// draws it: past the leaves a leaf height shifted, under them the mean as
/// its blocks round it.
pub fn face_height(cell: Zoom, top: i32) -> i32 {
    let d = cell as i32 - LEAF_CELL as i32;
    if d >= 0 {
        // Divided by 2^d toward zero, as the grid draws a face under sea
        // level.
        if top < 0 {
            -(-top >> d)
        } else {
            top >> d
        }
    } else {
        top << -d
    }
}

/// The bedrock's top under a face at `face` leaf cells, in blocks of `cell`.
pub fn rock(cell: Zoom, face: i32) -> i32 {
    blocks(cell, bedrock(face) as f32)
}

/// What the ground is in block `z` of a cell of `cell` whose look is `look`
/// and whose top is `top` blocks: by depth under its top, `void` over it. A
/// cell coarser than the leaves' is a summary of the faces under it, its
/// ground its face whole; the wall (`unknown`) is itself all through.
pub fn by_depth(cell: Zoom, look: u32, top: i32, z: i32) -> u32 {
    if z >= top {
        return VOID;
    }
    if cell < LEAF_CELL || look == UNKNOWN {
        return look;
    }
    let s = leaf_blocks(cell);
    // Water's filler is water, as sand's is sand: under a face it is the
    // same block, never dirt.
    let (filler, sandstone) = match look {
        SAND | BEACH => (SAND, SANDSTONE_DEPTH),
        WATER => (WATER, 0),
        _ => (SOIL, 0),
    };
    let face = top - s;
    let filled = face - FILLER_DEPTH * s;
    if z >= face {
        look
    } else if z >= filled {
        filler
    } else if z >= filled - sandstone * s {
        SANDSTONE
    } else if z >= rock(cell, face_height(cell, top)) {
        STONE
    } else {
        UNKNOWN
    }
}

/// What the ground is over some blocks of its cells, its tag a word, as a
/// GPU has no byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Lay {
    /// Over every top: none of it is ground.
    Air,
    /// Ground a cave may take, all of it: under every face block, over every
    /// bedrock, off the service area's wall; the caves' hash says which.
    Open,
    /// Some of it may be ground no cave takes: a face block, the bedrock,
    /// the wall.
    Kept,
}

/// What the ground is over the blocks `z0..z1` of `cell`, in cells whose
/// faces lie between `lo` and `hi` leaf cells and where the service area's
/// wall may stand or not: the ground's one rule of where it stands and where
/// a cave may take it, asked of one block or of the blocks under a node at
/// each zoom a reader asks within it. A top and a bedrock are the face's,
/// rounded to blocks as the grid rounds them.
pub fn lay(cell: Zoom, [lo, hi]: [i32; 2], wall: bool, [z0, z1]: [i32; 2]) -> Lay {
    let top = |h: i32| blocks(cell, h as f32);
    if z0 >= top(hi) {
        Lay::Air
    } else if !wall && z1 <= top(lo) - leaf_blocks(cell) && z0 >= rock(cell, hi) {
        Lay::Open
    } else {
        Lay::Kept
    }
}

/// Whether nothing under the node `(zoom, c)` fills at the zooms from `far`
/// to `near` a reader asks within it, by the ground's one rule (`lay`),
/// where the faces under it lie between `lo` and `hi` leaf cells: every
/// cell under it over the ground at each of those zooms, or ground a cave
/// may take with a cave through all of it (`hollow`). Coarser than a shard,
/// the Earth past the service area may be in it too, its face at sea level.
///
/// `lay` at each zoom, in one test. The node's bottom and top are whole
/// blocks of every zoom finer than it, an even count of each, and `blocks`
/// rounds a height to the nearest block, a tie to the even one: so a height
/// rounds to at most the bottom at a zoom exactly when it is at most half
/// that zoom's block over it, and the finest zoom asked says it for all of
/// them; while the top is a face block under a rounded height exactly when
/// that height is more than half a block over the top, and the coarsest
/// says it. One zoom's cells cannot be over the ground where another's are
/// under it, so one of the two holds for every zoom or neither does.
pub fn empty(area: &impl Area, shard: Shard, (zoom, c): (Zoom, [i32; 3]), [lo, hi]: [i32; 2], [far, near]: [Zoom; 2], hollow: bool) -> bool {
    let [lo, hi] = if zoom < SHARD_ZOOM { [lo.min(SEA_LEVEL), hi.max(SEA_LEVEL)] } else { [lo, hi] };
    let coarsest = if far > zoom + 1 { far } else { zoom + 1 };
    if coarsest > near {
        return true;
    }
    // Leaf cells cut 2^e a side, the finest asked's blocks where finer than
    // the leaves, so every height here is a whole count.
    let e = near.max(LEAF_CELL) - LEAF_CELL;
    let s = LEAF_CELL + e - zoom;
    let (bottom, top) = (c[2] << s, (c[2] + 1) << s);
    // Half a block of a zoom, in those units; none at the leaves or finer,
    // where a height is whole blocks.
    let half = |z: Zoom| if z < LEAF_CELL { 1 << (LEAF_CELL - z - 1 + e) } else { 0 };
    if hi << e <= bottom + half(near) {
        return true;
    }
    // Under a face block at every zoom: past half the coarsest's block over
    // the top, or at the leaves or finer, a leaf cell over it.
    let under = if coarsest < LEAF_CELL { lo << e > top + half(coarsest) } else { lo << e >= top + (1 << e) };
    let rock = bedrock(hi) << e;
    hollow && under && rock <= bottom + half(near) && !walled(area, shard, near, (zoom, [c[0], c[1]]))
}

/// Whether the block `z` of a cell at `cell` lies over the ground wherever
/// in the column the faces under it reach at most `hi` leaf cells: `blocks`
/// is monotone in a height, so a top rounded as the grid rounds it is at
/// most `hi`'s.
pub fn above(cell: Zoom, z: i32, hi: i32) -> bool {
    z >= blocks(cell, hi as f32)
}

/// A bound over every height a cell holds, i16's whole range.
pub const UNBOUNDED: [i32; 2] = [i16::MIN as i32, i16::MAX as i32];

/// The bound of no ground, lowest over highest: what joins as nothing.
pub const EMPTY: [i32; 2] = [i16::MAX as i32, i16::MIN as i32];

/// How the ground is held where a node reads it: the CPU's residency, a
/// GPU's window over the tiles it was handed.
pub trait Held: Area {
    /// The lowest and highest the ground's cells at `cell` reach over the
    /// node `(zoom, c)`, off the finest held tile they are read from
    /// (`window::bounded`); a bound no tighter than the heights' whole range
    /// (`UNBOUNDED`) where no square it reads is held, which only splits
    /// more; false where no tile over it is held.
    /// `place` is where the cells at `cell` of a node over it were found,
    /// or `Found::NONE`, and becomes where its own are (`window::step`).
    fn relief(&mut self, place: &mut Found, cell: Zoom, node: (Zoom, [i32; 3])) -> ([i32; 2], bool);
    /// The column of the block `c` at `zoom` as the grid reads it, the
    /// service area's edge laid on it (`Cell`), none where `shard` is known
    /// `inside` it; its cell where `place` found it, the zoom's cells'
    /// (`relief`'s), or found anew where `place` is `NONE`; false where it
    /// cannot be read.
    fn cell(&mut self, shard: Shard, place: Found, zoom: Zoom, c: [i32; 3]) -> (Cell, bool);
}

/// What the ground answers of a node (`node`): its kind, and a block's
/// look and its fold's four.
pub type Answer = In<(u32, [u32; 4])>;

/// The ground's answer for a node, the one way a descent asks the ground,
/// `cave` the caves there and `block` whether it is asked at its own zoom:
/// past the service area, the Earth's; else the node's own bound first, of
/// each zoom's tile its cells are read from (`Held::relief`) — a block over it is air,
/// else its cell's block (`Cell::block`), its look and its fold's four; a
/// coarser node nothing under fills at the zooms asked (`empty`) a block of
/// air its size, else split; where no tile over it is held, the lack.
/// `shard` and `place` are what a descent established over it, of the area
/// (`Shard`) and where the cells of a node over it were held
/// (`Held::relief`), each `UNKNOWN`/`NONE` where none did, and become the
/// node's. With the answer the cell its block was read off and where, none
/// (`Cell::NONE`, `Found::NONE`) where it read none.
pub fn node<H: Held>(held: &mut H, cave: Cave, shard: &mut Shard, place: &mut Found, (zoom, c): (Zoom, [i32; 3]), [far, near]: [Zoom; 2], block: bool) -> (Answer, Cell, Found) {
    let none = Found::NONE;
    let air = In {
        kind: Kind::Block,
        block: (VOID, [VOID; 4]),
    };
    let split = In {
        kind: Kind::Split,
        block: (VOID, [VOID; 4]),
    };
    let lacks = In {
        kind: Kind::Lacks,
        block: (VOID, [VOID; 4]),
    };
    // Past the area: a block's own Earth, or a node's where it is all over
    // sea level. No node a shard or finer straddles the area.
    *shard = shard.at(held, (zoom, [c[0], c[1]]));
    if shard.past() {
        let earth = earth(c[2]);
        let answer = if block {
            In {
                kind: Kind::Block,
                block: (earth, [earth; 4]),
            }
        } else if earth == VOID {
            air
        } else {
            split
        };
        return (answer, Cell::NONE, none);
    }
    // Coarser than any tile, no one index holds it.
    if zoom < GROUND.top() {
        return (split, Cell::NONE, none);
    }
    // A block's cells are its zoom's. A coarser node's are read at the
    // finest zoom asked within it, no finer than a tile at the node's own
    // zoom holds: heights fold by their mean, so the finest's bound holds
    // every coarser one's over the node, and what a tile at the node's zoom
    // flattened is not the world there.
    let fit = GROUND.grid_zoom(zoom);
    let read = if block {
        zoom
    } else if near < fit {
        near
    } else {
        fit
    };
    let (relief, ok) = held.relief(place, read, (zoom, c));
    if !ok {
        return (lacks, Cell::NONE, none);
    }
    if block {
        if above(zoom, c[2], relief[1]) {
            return (air, Cell::NONE, none);
        }
        let (column, ok) = held.cell(*shard, *place, zoom, c);
        if !ok {
            return (lacks, Cell::NONE, none);
        }
        let answer = In {
            kind: Kind::Block,
            block: column.block(zoom, c[2], cave.hollow()),
        };
        return (answer, column, *place);
    }
    let answer = if empty(held, *shard, (zoom, c), relief, [far, near], cave.whole(zoom) == (true, true)) {
        air
    } else {
        split
    };
    (answer, Cell::NONE, none)
}

/// A cell's ground, as the grid reads it: its look (`unknown` for the wall),
/// its top in blocks of its zoom, its fold's four looks, and whether the
/// service area's wall stands in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub look: u32,
    pub top: i32,
    pub fold: [u32; 4],
}

impl Cell {
    /// No ground: void at sea level, as a column no tile is read for.
    pub const NONE: Cell = Cell { look: VOID, top: 0, fold: [VOID; 4] };

    /// Whether it has ground at all: a look the catalog grounds, or the wall.
    pub fn grounded(&self) -> bool {
        self.look == UNKNOWN || Ground::grounds(self.look)
    }

    /// Whether a cave may take block `z` of the cell at `cell` (`lay`): ground
    /// under its face, over its bedrock, off the area's wall.
    pub fn opens(&self, cell: Zoom, z: i32) -> bool {
        let face = face_height(cell, self.top);
        lay(cell, [face, face], self.look == UNKNOWN, [z, z + 1]) == Lay::Open
    }

    /// Block `z` of the cell at `cell`, a cave's air where `hollow` and a
    /// cave may take it (`opens`): the ground's look and its fold's four,
    /// `void` for air.
    pub fn block(&self, cell: Zoom, z: i32, hollow: bool) -> (u32, [u32; 4]) {
        let air = (VOID, [VOID; 4]);
        if !self.grounded() {
            return air;
        }
        let g = by_depth(cell, self.look, self.top, z);
        if g == VOID || hollow && self.opens(cell, z) {
            return air;
        }
        let folds = cell >= LEAF_CELL || g != self.look;
        (g, if folds { [g; 4] } else { self.fold })
    }
}
