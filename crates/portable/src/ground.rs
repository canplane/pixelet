// pxl-portable/ground.rs

//! gpu-portable: the ground's column at a cell, the one rule a node's ground
//! is answered by (`node`) before what overwrites it: a cell's look and
//! height off the window drawing it (`window::ground`), and its blocks by
//! depth with a cave's air where a cave may take them. What overwrites it
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
use crate::lattice::TILE_CELLS;
use crate::lattice::{CELL_CENTER, GROUND};
use crate::window::Found;
use crate::{Zoom, LEAF_CELL, LEAF_ZOOM, SEA_LEVEL, SHARD_ZOOM, UNIT_ZOOM};

// The catalog's looks this rule names, read into a word.
/// The catalog's void and unknown as the words a look is read into.
pub const VOID: u32 = sentinel::void as u32;
pub const UNKNOWN: u32 = sentinel::unknown as u32;
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

/// Leaf cells over the ground or sea level, whichever is higher, a block
/// may reach: Minecraft's 320 over its −64, held from sea level so a shore
/// is not buried.
pub const HEADROOM: i32 = 128;

/// The highest a block may reach over a face at `face` leaf cells, in leaf
/// cells: every edit's ceiling, as `bedrock` is its floor.
pub fn ceiling(face: i32) -> i32 {
    face.max(SEA_LEVEL) + HEADROOM
}

/// The height the surface draws at the center of a tile's cell `(x, y)`,
/// in leaf cells, off its padded elevation samples `ele` (`ele_index`): the
/// one producer of a cell's height, which a GPU runs once a cell of a held
/// tile (`heights_cs`), and the CPU reads back.
pub fn height(ele: impl Fn(usize) -> i32, [x, y]: [i32; 2]) -> i32 {
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

/// What the windows bound under a node, each source its own: the ground's
/// faces, the lowest and highest, and the leaf cells the ways fill, the
/// lowest and past the highest (`window::LEVEL_WORDS`, `fill::level`).
pub type Bounds = [[i32; 2]; 2];

/// Bounds as their two bound words.
pub fn bounds_word(b: Bounds) -> [i32; 2] {
    [bound_word(b[0]), bound_word(b[1])]
}

/// `bounds_word`'s bounds.
pub fn bounds_of(w: [i32; 2]) -> Bounds {
    [bound_of(w[0]), bound_of(w[1])]
}

/// Two bounds joined, each source's its own.
pub fn joined(a: Bounds, b: Bounds) -> Bounds {
    [[a[0][0].min(b[0][0]), a[0][1].max(b[0][1])], [a[1][0].min(b[1][0]), a[1][1].max(b[1][1])]]
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
/// bake folds a tile's (pxl-spec bake/fold.rs `fold_ground`).
pub fn own(w: u32, cell: Zoom) -> u32 {
    majority(looks(w), cell)
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

/// Where the ground may fill the node `(zoom, c)` along z, at the zooms
/// from `far` to `near` a reader asks within it, where the faces under it
/// at those zooms lie between `lo` and `hi` leaf cells (`node`'s relief,
/// every one of those zooms' joined): from its lowest to past its highest,
/// in leaf cells, by the ground's one rule (`blocks`, `Cell::opens`).
/// Coarser than a shard, the Earth past the service area may be in it too,
/// its face at sea level.
///
/// Over it, past the highest top any of those zooms rounds `hi` to: a
/// coarser zoom's block may round past a finer's. Under it, the lowest
/// face's top block at those zooms, the ground a cave leaves there, where a
/// cave takes all of the node (`hollow`), no bedrock is in it and no wall
/// is about it; else from the world's bottom. Nothing (`EMPTY`) where no
/// zoom is asked within it.
///
/// What fills the node is what of it lies in the slab (`meets`): its bottom
/// and top are whole blocks of every zoom finer than it, an even count of
/// each, and `blocks` rounds a height to the nearest block, a tie to the
/// even one, so a zoom's top is at most the bottom exactly when the face
/// is at most half that zoom's block over it, the finest zoom's saying it
/// for all of them; and a zoom's lowest top block is at least the top
/// exactly when the face is more than half that zoom's block over it, the
/// coarsest's saying it.
pub fn slab(shard: Shard, (zoom, c): (Zoom, [i32; 3]), [lo, hi]: [i32; 2], [far, near]: [Zoom; 2], hollow: bool) -> [i32; 2] {
    let [lo, hi] = if zoom < SHARD_ZOOM { [lo.min(SEA_LEVEL), hi.max(SEA_LEVEL)] } else { [lo, hi] };
    // No bedrock in it: the bedrock's top, rounded at `near` as a face is,
    // at most its bottom.
    let rock = face_height(near, blocks(near, bedrock(hi) as f32));
    let caved = hollow && !meets([UNBOUNDED[0], rock], (zoom, c[2]), near) && !walled(shard, near, (zoom, [c[0], c[1]]));
    let [mut under, mut over] = if caved { EMPTY } else { [UNBOUNDED[0], EMPTY[1]] };
    let mut z = if far > zoom + 1 { far } else { zoom + 1 };
    while z <= near {
        let top = face_height(z, blocks(z, hi as f32));
        over = if top > over { top } else { over };
        if caved {
            let floor = face_height(z, blocks(z, lo as f32) - leaf_blocks(z));
            under = if floor < under { floor } else { under };
        }
        z += 1;
    }
    [under, over]
}

/// Whether the leaf cells `[lo, hi)` along z meet the node `(zoom, z)`, read
/// to `near`: past the leaves a leaf cell is cut 2^e a side, so every
/// height is a whole count.
pub fn meets([lo, hi]: [i32; 2], (zoom, z): (Zoom, i32), near: Zoom) -> bool {
    let e = near.max(LEAF_CELL) - LEAF_CELL;
    let s = LEAF_CELL + e - zoom;
    z << s < hi << e && (z + 1) << s > lo << e
}

/// Whether the block `z` of a cell at `cell` lies over the ground wherever
/// in the column the faces under it reach at most `hi` leaf cells: `blocks`
/// is monotone in a height, so a top rounded as the grid rounds it is at
/// most `hi`'s.
pub fn above(cell: Zoom, z: i32, hi: i32) -> bool {
    z >= blocks(cell, hi as f32)
}

/// What two bounds hold together: the lower of their lowest, the higher of
/// their highest (`EMPTY` joins as nothing).
pub fn join(a: [i32; 2], b: [i32; 2]) -> [i32; 2] {
    [if a[0] < b[0] { a[0] } else { b[0] }, if a[1] > b[1] { a[1] } else { b[1] }]
}

/// A bound over every height a cell holds, i16's whole range: a relief, the
/// lowest and highest face heights, both held. Not a slab of what may fill
/// along z, half-open (`world::ANY`), which is another fact in another unit.
pub const UNBOUNDED: [i32; 2] = [i16::MIN as i32, i16::MAX as i32];

/// The bound of no ground, lowest over highest: what joins as nothing.
pub const EMPTY: [i32; 2] = [i16::MAX as i32, i16::MIN as i32];

/// How the ground is held where a node reads it: the windows, as the CPU
/// read them back or a GPU filled them, read the one way (`window::ground`,
/// `window::bound_over`).
pub trait Held: Area {
    /// The lowest and highest the ground's cells at `cell` reach over the
    /// node `(zoom, c)`, and the ways' over them, apart (`Bounds`), off the
    /// finest held tile they are read from (`window::bound_over`); bounds no
    /// tighter than the heights' whole range (`UNBOUNDED`) where no square
    /// it reads is held, which only splits more; false where no tile over
    /// it is held.
    /// `place` is where the cells at `cells`' finest of a node over it were
    /// found, or `Found::NONE`, and becomes where its own are
    /// (`window::step`). Read over every zoom from `cells`' coarsest to its
    /// finest (`window::bound_over`).
    fn relief(&mut self, place: &mut Found, cells: [Zoom; 2], node: (Zoom, [i32; 3])) -> (Bounds, bool);
    /// The column of the block `c` at `zoom` off the windows
    /// (`window::ground`), the service area's edge laid on it as its fill
    /// laid it (`Cell`); its cell where `place` found it, the zoom's cells'
    /// (`relief`'s), or found anew where `place` is `NONE`.
    fn cell(&mut self, place: Found, zoom: Zoom, c: [i32; 3]) -> Cell;
}

/// What the ground answers of a node (`node`): its kind, and a block's
/// look.
pub type Answer = In<u32>;

/// The ground's answer for a node, the one way a descent asks the ground,
/// `cave` the caves there and `block` whether it is asked at its own zoom:
/// past the service area, the Earth's; else the node's own bound first, of
/// each zoom's tile its cells are read from (`Held::relief`) — a block over it is air,
/// else its cell's block (`Cell::block`), its look; a
/// coarser node no part of which lies where the ground may fill at the zooms asked (`slab`, `meets`) a block of
/// air its size, else split; where no tile over it is held, a block is the
/// lack and a coarser node split, not known empty.
/// `shard` is what is known of the area at the node, known a shard or
/// finer: its asker's, which asks it once at its entry (`Shard::at`).
/// `place` is where the cells of a node over it were held
/// (`Held::relief`), `NONE` where none were, and becomes the node's. With
/// the answer the cell its block was read off and where it was read, what
/// the ways write over it read off (`window::ways_over`); none
/// (`Cell::NONE`, `Found::NONE`) where it read none, as over the ground
/// where the ways miss the block (`ways::slab`); and the ways' bound read
/// with its own (`Held::relief`), `EMPTY` where none was.
pub fn node<H: Held>(held: &mut H, cave: Cave, shard: Shard, place: &mut Found, (zoom, c): (Zoom, [i32; 3]), [far, near]: [Zoom; 2], block: bool) -> (Answer, Cell, Found, [i32; 2]) {
    let none = Found::NONE;
    let air = In { kind: Kind::Block, block: VOID };
    // A split no bound tells the slab of holds anything anywhere along z.
    let split = In {
        kind: Kind::Split,
        block: crate::world::slab_word(crate::world::ANY),
    };
    let lacks = In { kind: Kind::Lacks, block: VOID };
    // Past the area: a block's own Earth, or a node's where it is all over
    // sea level. No node a shard or finer straddles the area.
    if shard.past() {
        let earth = earth(c[2]);
        let answer = if block {
            In { kind: Kind::Block, block: earth }
        } else if earth == VOID {
            air
        } else {
            split
        };
        return (answer, Cell::NONE, none, EMPTY);
    }
    // Coarser than any tile, no one index holds it.
    if zoom < GROUND.top() {
        return (split, Cell::NONE, none, EMPTY);
    }
    // A block's cells are its zoom's. A coarser node's are every zoom's
    // asked within it, from the coarsest to `near`: each zoom's cells are
    // its own tile's, whose face, its own samples' bicubic, no other zoom's
    // bounds, above or below, so the bound is every one's joined, read once
    // a window (`Held::relief`).
    let first = if far > zoom + 1 { far } else { zoom + 1 };
    let cells = if block { [zoom, zoom] } else { [if first > near { near } else { first }, near] };
    let ([relief, ways], ok) = held.relief(place, cells, (zoom, c));
    // Not held at a zoom asked: a block is the lack; a coarser node is not
    // known empty, so it is split, and a ray stops only at a block it meets
    // not held.
    if !ok {
        return (if block { lacks } else { split }, Cell::NONE, none, EMPTY);
    }
    if block {
        // Its column read where the ground or the ways may fill it.
        if above(zoom, c[2], relief[1]) && !meets(crate::ways::slab(ways, [zoom, zoom]), (zoom, c[2]), zoom) {
            return (air, Cell::NONE, none, ways);
        }
        let column = held.cell(*place, zoom, c);
        let block = column.block(zoom, c[2], || cave.hollow());
        let answer = In { kind: Kind::Block, block };
        return (answer, column, *place, ways);
    }
    // Air where none of it lies where the ground may fill, else a split
    // and where along z.
    let (known, hollow) = cave.whole(zoom);
    let s = slab(shard, (zoom, c), relief, [far, near], known && hollow);
    let answer = if meets(s, (zoom, c[2]), near) {
        In {
            kind: Kind::Split,
            block: crate::world::slab_word(s),
        }
    } else {
        air
    };
    (answer, Cell::NONE, none, ways)
}

/// A cell's ground, as a column reads it: its look (`unknown` for the
/// service area's wall), its top in blocks of its zoom, the four looks its
/// fold took (`looks`, `dx + 2·dy`), and its runs among its square's, first
/// to past its last
/// (`window::column_first` of its column's word and the next's), which the
/// visit that read it reads what the ways write in it off
/// (`window::ways_over`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub look: u32,
    pub top: i32,
    pub fold: [u32; 4],
    pub runs: [u32; 2],
}

impl Cell {
    /// No ground: void at sea level, as a column no tile is read for.
    pub const NONE: Cell = Cell {
        look: VOID,
        top: 0,
        fold: [VOID; 4],
        runs: [0; 2],
    };

    /// Whether it has ground at all: a look the catalog grounds, or the wall.
    pub fn grounded(&self) -> bool {
        self.look == UNKNOWN || Ground::grounds(self.look)
    }

    /// Whether a cave may take block `z` of the cell at `cell`: the ground's
    /// one rule of where a cave may take it, ground under its face block, over
    /// its bedrock, off the area's wall, the face and the bedrock rounded to
    /// blocks as the column rounds them.
    fn opens(&self, cell: Zoom, z: i32) -> bool {
        let face = face_height(cell, self.top);
        self.look != UNKNOWN && z < blocks(cell, face as f32) - leaf_blocks(cell) && z >= rock(cell, face)
    }

    /// The four looks block `g` of the cell at `cell` is drawn by: where it
    /// is the cell's own look at a cell coarser than the leaves', the four
    /// its fold took, whose mean it is drawn as, "the 2 × 2 … by design"
    /// (doc/minutes/20260928-what-is-drawn-is-what-the-screen-covers.md);
    /// else, a stratum under it or a leaf's cell, `g` four times. A way
    /// written over it is its own look four times (`world::node`).
    pub fn four(&self, cell: Zoom, g: u32) -> [u32; 4] {
        if cell >= LEAF_CELL || g != self.look {
            [g; 4]
        } else {
            self.fold
        }
    }

    /// Block `z` of the cell at `cell`, a cave's air where a cave may take
    /// it (`opens`) and `hollow` says one does, asked only there: the
    /// ground's look, its fold's majority at a coarser cell, as what is
    /// placed is its fold's (doc/minutes/20261005-the-ground-is-a-carto-palette.md,
    /// "One value space"), `void` for air.
    pub fn block(&self, cell: Zoom, z: i32, hollow: impl FnOnce() -> bool) -> u32 {
        if !self.grounded() {
            return VOID;
        }
        let g = by_depth(cell, self.look, self.top, z);
        if g == VOID || self.opens(cell, z) && hollow() {
            return VOID;
        }
        g
    }
}
