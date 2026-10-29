// pxl-spec-core/ground.rs

//! gpu-portable: the ground's column at a cell, the one rule a node's ground
//! is answered by (`node`) before what overwrites it: a cell's look and
//! height off the window drawing it (`window::ground`), and its cells by
//! depth with a cave's air where a cave may take them. What overwrites it
//! is not here: the ways by their own rule (`way.rs`), and what is placed
//! by its own (`placed.rs`).
//!
//! A look and a height are stored as a byte and an i16 and read into a word
//! here, since WGSL has no narrower integer.
//!
//! Heights are f32 here, as WGSL has them. WGSL's f32 is not strictly IEEE (a
//! division's last place, a contraction into a fused multiply-add), so a
//! height a CPU and a GPU compute from this one source can differ in the
//! last bit where it falls on an eighth of a cell.

use crate::answer::{
    bound_of, bound_word, box_word, eighths, BoxWords, In, Kind, BOX_BITS, BOX_NONE, BOX_STEPS,
    BOX_WHOLE, FULL,
};
use crate::area::{earth, is_walled, Area, Shard};
use crate::cave::Cave;
use crate::cube::{ground, sentinel, Ground};
use crate::lattice::CELL_CENTER;
use crate::lattice::TILE_CELLS;
use crate::param::ground::BEDROCK_DEPTH;
use crate::param::ground::FILLER_DEPTH;
use crate::param::ground::HEADROOM;
use crate::param::ground::SANDSTONE_DEPTH;
use crate::window::Found;
use crate::{Zoom, LEAF_CELL, LEAF_ZOOM, SEA_LEVEL, SHARD_ZOOM, TOP_ZOOM, UNIT_ZOOM};

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
pub const fn ele_idx(x: u32, y: u32) -> usize {
    (y * ELE_PADDED_SPAN + x) as usize
}

/// The bedrock's top under a face at `face` leaf cells, in leaf cells: the
/// lowest a cube may reach there, every walk's, edit's and column's.
pub fn bedrock(face: i32) -> i32 {
    face.min(SEA_LEVEL) - BEDROCK_DEPTH
}

/// The highest a cube may reach over a face at `face` leaf cells, in leaf
/// cells: every edit's ceiling, as `bedrock` is its floor.
pub fn ceiling(face: i32) -> i32 {
    face.max(SEA_LEVEL) + HEADROOM
}

/// The height the surface draws at the center of a tile's cell `(x, y)`,
/// in leaf cells, off its padded elevation samples `ele` (`ele_idx`): the
/// one producer of a cell's height, which a GPU runs once a cell of a held
/// tile (`height_cs`), and the CPU reads back.
pub fn height(ele: impl Fn(usize) -> i32, [x, y]: [i32; 2]) -> i32 {
    bicubic(ele, x as f32 + CELL_CENTER, y as f32 + CELL_CENTER)
}

/// A tile's padded samples as a GPU holds them, 2 a word.
pub const ELE_WORDS: u32 = (ELE_PADDED_SPAN * ELE_PADDED_SPAN).div_ceil(2);

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
    [
        [a[0][0].min(b[0][0]), a[0][1].max(b[0][1])],
        [a[1][0].min(b[1][0]), a[1][1].max(b[1][1])],
    ]
}

/// The bits a look takes in a cell's word, four to the word.
pub const LOOK_BITS: u32 = 8;

/// A cell's word's four looks, `dx + 2·dy`.
pub fn looks(word: u32) -> [u32; 4] {
    let m = (1 << LOOK_BITS) - 1;
    [
        word & m,
        word >> LOOK_BITS & m,
        word >> (2 * LOOK_BITS) & m,
        word >> (3 * LOOK_BITS),
    ]
}

/// Four looks as a cell's word.
pub fn word([a, b, c, d]: [u32; 4]) -> u32 {
    a | b << LOOK_BITS | c << (2 * LOOK_BITS) | d << (3 * LOOK_BITS)
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
    [
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2,
    ]
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
            let v = ele(ele_idx(held(x0 + i), py));
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

/// The value filling more than half of a 2×2 (or 2×2×2) group, `v[dx + 2·dy +
/// 4·dz]`; otherwise the tie child's. The tie corner alternates with the
/// parent cell's zoom, since a fixed corner drifts the map half a coarse cell a step.
pub fn majority<T: Copy + Eq, const N: usize>(v: [T; N], parent: Zoom) -> T {
    let tie = v[if parent & 1 == 1 { 0 } else { N - 1 }];
    let cnt = |x: T| {
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
        let n = cnt(x);
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

/// A height in leaf cells as cells of `cell` (`top`).
pub fn cells(cell: Zoom, h: f32) -> i32 {
    top(cell, h).0
}

/// A height in leaf cells as cells of `cell`, and the share of the top one
/// it fills, in `answer::FULL`'s units: shifted past the leaves, every
/// cell whole; under them, the mean in cells, its top the cell it ends
/// in where it fills an eighth of it or more (`answer::eighths`), the share
/// of it filled kept there, else the cell under it, whole. So a coarse
/// column's ground is the volume its mean height fills, as a cell of it is
/// drawn: whole under its top, its top a box its share's volume.
pub fn top(cell: Zoom, h: f32) -> (i32, u32) {
    let d = cell as i32 - LEAF_CELL as i32;
    if d >= 0 {
        return ((h as i32) << d, FULL);
    }
    let q = h / (1 << -d) as f32;
    let under = libm::floorf(q);
    let part = ((q - under) * FULL as f32) as u32;
    if eighths(part) == 0 {
        (under as i32, FULL)
    } else {
        (under as i32 + 1, part)
    }
}

/// Cells of `cell` a leaf cell is tall: one where a cell folds the leaves.
pub fn leaf_cells(cell: Zoom) -> i32 {
    1 << (cell as i32 - LEAF_CELL as i32).max(0)
}

/// The face of a top of `top` cells of `cell`, in leaf cells, as the grid
/// draws it: past the leaves a leaf height shifted, under them the mean as
/// its cells round it.
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

/// The face cell of a column whose face is `face` leaf cells, its first
/// cell in cells of `cell`: under it, the ground under its face, which a
/// cave may take (`Cell::opens`) and a way's bore is lined in
/// (pxl-spec-core `way::lined`).
pub fn face_cell(cell: Zoom, face: i32) -> i32 {
    cells(cell, face as f32) - leaf_cells(cell)
}

/// The bedrock's top under a face at `face` leaf cells, in cells of `cell`.
pub fn rock(cell: Zoom, face: i32) -> i32 {
    cells(cell, bedrock(face) as f32)
}

/// What the ground is in cell `z` of a column of `cell` whose look is `look`
/// and whose top is `top` cells: by depth under its top, `void` over it. A
/// column coarser than the leaves' is a summary of the faces under it, its
/// ground its face whole; the wall (`unknown`) is itself all through.
pub fn by_depth(cell: Zoom, look: u32, top: i32, z: i32) -> u32 {
    if z >= top {
        return VOID;
    }
    if cell < LEAF_CELL || look == UNKNOWN {
        return look;
    }
    let s = leaf_cells(cell);
    // Water's filler is water, as sand's is sand: under a face it is the
    // same cell, never dirt.
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
/// in leaf cells, by the ground's one rule (`cells`, `Cell::opens`).
/// Coarser than a shard, the Earth past the service area may be in it too,
/// its face at sea level.
///
/// Over it, past the highest top any of those zooms rounds `hi` to: a
/// coarser zoom's cell may round past a finer's. Under it, the lowest
/// face's top cell and the one under it at those zooms, the ground a cave
/// leaves there (`Cell::opens`), where a
/// cave takes all of the node (`hollow`), no bedrock is in it and no wall
/// is about it; else from the world's bottom. Nothing (`EMPTY`) where no
/// zoom is asked within it.
///
/// What fills the node is what of it lies in the slab (`meets`): its bottom
/// and top are whole cells of every zoom finer than it, an even count of
/// each, and `cells` ends a height at the cell it fills an eighth of or
/// more (`top`), so a zoom's top is at most the bottom exactly when the
/// face is under an eighth of that zoom's cell over it, the finest zoom's
/// saying it for all of them. A top kept short of whole still lies in it
/// whole, which only splits more.
pub fn slab(
    shard: Shard,
    (zoom, c): (Zoom, [i32; 3]),
    [lo, hi]: [i32; 2],
    [far, near]: [Zoom; 2],
    hollow: bool,
) -> [i32; 2] {
    let [lo, hi] = if zoom < SHARD_ZOOM {
        [lo.min(SEA_LEVEL), hi.max(SEA_LEVEL)]
    } else {
        [lo, hi]
    };
    // No bedrock in it: the bedrock's top, rounded at `near` as a face is,
    // at most its bottom.
    let rock = face_height(near, cells(near, bedrock(hi) as f32));
    let caved = hollow
        && !meets([UNBOUNDED[0], rock], (zoom, c[2]), near)
        && !is_walled(shard, near, (zoom, [c[0], c[1]]));
    let [mut under, mut over] = if caved {
        EMPTY
    } else {
        [UNBOUNDED[0], EMPTY[1]]
    };
    let mut z = coarsest(zoom, far);
    while z <= near {
        let top = face_height(z, cells(z, hi as f32));
        over = if top > over { top } else { over };
        if caved {
            // Its top cell, and the cell under it where the top is a box
            // (`Cell::opens`).
            let floor = face_height(z, cells(z, lo as f32) - 2 * leaf_cells(z));
            under = if floor < under { floor } else { under };
        }
        z += 1;
    }
    [under, over]
}

/// The coarsest zoom asked under a node at `zoom` coarser than asked, `far`
/// the coarsest any ray asks within it: never its own, since it is coarser,
/// so no coarser than its children's. Every source's bound over a node is
/// read from here to the finest asked.
pub fn coarsest(zoom: Zoom, far: Zoom) -> Zoom {
    if far > zoom + 1 {
        far
    } else {
        zoom + 1
    }
}

/// Whether the leaf cells `[lo, hi)` along z meet the node `(zoom, z)`, read
/// to `near`: past the leaves a leaf cell is cut 2^e a side, so every
/// height is a whole count.
pub fn meets([lo, hi]: [i32; 2], (zoom, z): (Zoom, i32), near: Zoom) -> bool {
    let e = near.max(LEAF_CELL) - LEAF_CELL;
    let s = LEAF_CELL + e - zoom;
    z << s < hi << e && (z + 1) << s > lo << e
}

/// A bound over every height a cell holds, i16's whole range: a relief, the
/// lowest and highest face heights, both held. Not a span of what may fill
/// along z, half-open (`slab`), which is another fact in another unit.
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
    /// tighter than the heights' whole range (`UNBOUNDED`) where no brick
    /// it reads is held, which only splits more; false where no tile over
    /// it is held.
    /// `place` is where the cells at `cells`' finest of a node over it were
    /// found, or `Found::NONE`, and becomes where its own are
    /// (`window::step`). Read over every zoom from `cells`' coarsest to its
    /// finest (`window::bound_over`).
    fn relief(
        &mut self,
        place: &mut Found,
        cells: [Zoom; 2],
        node: (Zoom, [i32; 3]),
    ) -> (Bounds, bool);
    /// The column of the cell `c` at `zoom` off the windows
    /// (`window::ground`), the service area's edge laid on it as its fill
    /// laid it (`Cell`); its cell where `place` found it, the zoom's cells'
    /// (`relief`'s), or found anew where `place` is `NONE`.
    fn cell(&mut self, place: Found, zoom: Zoom, c: [i32; 3]) -> Cell;
}

/// What the ground answers of a node (`node`): its kind, and a cell's
/// asset and look.
pub type Answer = In<[u32; 2]>;

/// The ground's answer for a node, the one way a descent asks the ground,
/// `cave` the caves there and `is_cell` whether it is asked at its own zoom:
/// past the service area, the Earth's; else the node's own bound first, of
/// each zoom's tile its cells are read from (`Held::relief`) — at its own zoom,
/// over it is air, else its column's asset (`Cell::asset`), its look; a
/// coarser node no part of which lies where the ground may fill at the zooms asked (`slab`, `meets`) a cube of
/// air its size, else split; where no tile over it is held, a node at its own
/// zoom is the lack and a coarser node split, not known empty.
/// `shard` is what is known of the area at the node, known a shard or
/// finer: its asker's, which asks it once at its entry (`Shard::at`).
/// `place` is where the cells of a node over it were held
/// (`Held::relief`), `NONE` where none were, and becomes the node's: where
/// a cell's column was read, which the ways write over it are read at
/// (pxl-spec-core `world::node`), and `NONE` past the area, where nothing
/// is read. With the answer the column its cell was read off, none
/// (`Cell::NONE`) for a coarser node; and the ways' bound read with its own
/// (`Held::relief`), `EMPTY` where none was.
pub fn node<H: Held>(
    held: &mut H,
    cave: Cave,
    shard: Shard,
    place: &mut Found,
    (zoom, c): (Zoom, [i32; 3]),
    [far, near]: [Zoom; 2],
    is_cell: bool,
) -> (Answer, Cell, [i32; 2]) {
    let air = In {
        kind: Kind::Cell,
        asset: [VOID, 0],
    };
    // A split no bound tells the span of holds anything anywhere in it.
    let split = In {
        kind: Kind::Split,
        asset: BOX_WHOLE,
    };
    let lacks = In {
        kind: Kind::Lacks,
        asset: [VOID, 0],
    };
    // Past the area: a cell's own Earth, or a node's where it is all over
    // sea level. No node a shard or finer straddles the area.
    if shard.past() {
        let earth = earth(c[2]);
        let answer = if is_cell {
            In {
                kind: Kind::Cell,
                asset: [earth, 0],
            }
        } else if earth == VOID {
            air
        } else {
            split
        };
        *place = Found::NONE;
        return (answer, Cell::NONE, EMPTY);
    }
    // Coarser than any tile, no one index holds it.
    if zoom < TOP_ZOOM {
        return (split, Cell::NONE, EMPTY);
    }
    // A cell's columns are its zoom's. A coarser node's are every zoom's
    // asked within it, from the coarsest to `near`: each zoom's cells are
    // its own tile's, whose face, its own samples' bicubic, no other zoom's
    // bounds, above or below, so the bound is every one's joined, read once
    // a window (`Held::relief`).
    let first = coarsest(zoom, far);
    let cells = if is_cell {
        [zoom, zoom]
    } else {
        [if first > near { near } else { first }, near]
    };
    let ([relief, ways], ok) = held.relief(place, cells, (zoom, c));
    // Not held at a zoom asked: a cell is the lack; a coarser node is not
    // known empty, so it is split, and a ray stops only at a cell it meets
    // not held.
    if !ok {
        return (if is_cell { lacks } else { split }, Cell::NONE, EMPTY);
    }
    if is_cell {
        let column = held.cell(*place, zoom, c);
        let asset = column.asset(zoom, c[2], || cave.hollow());
        return (
            In {
                kind: Kind::Cell,
                asset: [asset, 0],
            },
            column,
            ways,
        );
    }
    // Air where none of it lies where the ground may fill, else a split
    // and its box, the span along z it may fill in the node's steps.
    let (known, hollow) = cave.whole(zoom);
    let s = slab(shard, (zoom, c), relief, [far, near], known && hollow);
    let answer = if meets(s, (zoom, c[2]), near) {
        In {
            kind: Kind::Split,
            asset: box_along_z(s, (zoom, c[2])),
        }
    } else {
        air
    };
    (answer, Cell::NONE, ways)
}

/// A cell's ground, as a column reads it: its look (`unknown` for the
/// service area's wall), its top in cells of its zoom and the share of
/// its top cell it fills (`top`), and the four looks its fold took
/// (`looks`, `dx + 2·dy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub look: u32,
    pub top: i32,
    pub part: u32,
    pub fold: [u32; 4],
}

impl Cell {
    /// No ground: void at sea level, as a column no tile is read for.
    pub const NONE: Cell = Cell {
        look: VOID,
        top: 0,
        part: FULL,
        fold: [VOID; 4],
    };

    /// The share of cell `z` its ground fills, in `answer::FULL`'s
    /// units, where a cell of it fills there (`asset`): whole under its
    /// top cell, its part there (`top`).
    pub fn cover(&self, z: i32) -> u32 {
        if z == self.top - 1 {
            self.part
        } else {
            FULL
        }
    }

    /// Whether it has ground at all: a look the catalog grounds, or the wall.
    pub fn is_grounded(&self) -> bool {
        self.look == UNKNOWN || Ground::grounds(self.look)
    }

    /// Whether a cave may take cell `z` of the column at `cell`: the ground's
    /// one rule of where a cave may take it, ground under its face cell, over
    /// its bedrock, off the area's wall, the face and the bedrock rounded to
    /// cells as the column rounds them. Under a top kept short of whole
    /// is ground, as under a shrunk box, so the cell under it is not taken
    /// either.
    fn opens(&self, cell: Zoom, z: i32) -> bool {
        let face = face_height(cell, self.top);
        let under = face_cell(cell, face) - (self.part < FULL) as i32;
        self.look != UNKNOWN && z < under && z >= rock(cell, face)
    }

    /// The four looks asset `g` of the column at `cell` is drawn by: where it
    /// is the cell's own look at a cell coarser than the leaves', the four
    /// its fold took, whose mean it is drawn as, as the cell reads from afar;
    /// else, a stratum under it or a leaf's cell, `g` four times. A way written over it is its own look four
    /// times (`world::node`).
    pub fn four(&self, cell: Zoom, g: u32) -> [u32; 4] {
        if cell >= LEAF_CELL || g != self.look {
            [g; 4]
        } else {
            self.fold
        }
    }

    /// The asset in cell `z` of the column at `cell`, a cave's air where a cave may take
    /// it (`opens`) and `hollow` says one does, asked only there: the
    /// ground's look, its fold's majority at a coarser cell, as what is
    /// placed is its fold's, `void` for air.
    pub fn asset(&self, cell: Zoom, z: i32, hollow: impl FnOnce() -> bool) -> u32 {
        if !self.is_grounded() {
            return VOID;
        }
        let g = by_depth(cell, self.look, self.top, z);
        if g == VOID || self.opens(cell, z) && hollow() {
            return VOID;
        }
        g
    }
}

const _: () = assert!(
    crate::LEAF_CELL == crate::UNIT_ZOOM,
    "a span's leaf cell is the world's unit"
);
const _: () = assert!(
    -(1i32 << (crate::HEIGHT_BITS - 1)) == i16::MIN as i32
        && (1i32 << (crate::HEIGHT_BITS - 1)) - 1 == i16::MAX as i32,
    "the world's z, in leaf cells, is i16's range"
);

/// The leaf cells `[lo, hi)` along z of the node `(zoom, z)`, as its box:
/// its steps along z they meet, rounded out, and the node whole across.
/// A node finer than a leaf cell holds it whole; nothing, or a span the node
/// does not meet, none (`BOX_NONE`).
pub fn box_along_z([lo, hi]: [i32; 2], (zoom, z): (Zoom, i32)) -> BoxWords {
    if lo >= hi {
        return BOX_NONE;
    }
    if zoom >= crate::LEAF_CELL {
        return BOX_WHOLE;
    }
    // The step of the node a leaf cell `v` lies in, as the node's own
    // steps count it: `v` scaled by `BOX_STEPS` / side, less the node's first.
    let shift = crate::LEAF_CELL - zoom;
    let step = |v: i32| -> i32 {
        if shift >= BOX_BITS {
            (v >> (shift - BOX_BITS)) - (z << BOX_BITS)
        } else {
            (v << (BOX_BITS - shift)) - (z << BOX_BITS)
        }
    };
    // The span's first step and past its last, as the node counts them:
    // past the step its last leaf cell lies in, as a leaf cell a step or
    // wider ends on a step's edge of its own.
    let (first, past) = (
        step(lo),
        if shift >= BOX_BITS {
            step(hi - 1) + 1
        } else {
            step(hi)
        },
    );
    // A span the node does not meet is none of its box.
    if past <= 0 || first >= BOX_STEPS as i32 {
        return BOX_NONE;
    }
    let held = |e: i32, lo: i32, hi: i32| {
        if e < lo {
            lo
        } else if e > hi {
            hi
        } else {
            e
        }
    };
    let (first, past) = (
        held(first, 0, BOX_STEPS as i32 - 1) as u32,
        held(past, 1, BOX_STEPS as i32) as u32,
    );
    box_word([0, 0, first], [BOX_STEPS, BOX_STEPS, past])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::box_of;

    /// A z span becomes the steps of the node it meets, rounded out, the
    /// node whole across; none where the node does not meet it.
    #[test]
    fn z_span_rounded_out() {
        let zoom = crate::LEAF_CELL - 6;
        // The node of side 64 leaf cells over [128, 192), 16 steps a leaf
        // cell: [138, 140) is its steps [160, 192).
        assert_eq!(
            box_of(box_along_z([138, 140], (zoom, 2))),
            ([0, 0, 160], [1024, 1024, 192])
        );
        assert_eq!(
            box_of(box_along_z([100, 300], (zoom, 2))),
            ([0, 0, 0], [1024, 1024, 1024])
        );
        assert_eq!(box_along_z([5, 5], (zoom, 0)), BOX_NONE);
        // A span wholly under or over the node is none of it.
        assert_eq!(box_along_z([0, 128], (zoom, 2)), BOX_NONE);
        assert_eq!(box_along_z([192, 300], (zoom, 2)), BOX_NONE);
        // Under sea level: the node over [-64, 0), the span [-10, -9) its
        // steps [864, 880).
        assert_eq!(
            box_of(box_along_z([-10, -9], (zoom, -1))),
            ([0, 0, 864], [1024, 1024, 880])
        );
        // A root's node, its z the world's whole: one step holds it.
        assert_eq!(
            box_of(box_along_z([-5, 7], (0, 0))),
            ([0, 0, 0], [1024, 1024, 1])
        );
        assert_eq!(
            box_of(box_along_z([-5, 7], (0, -1))),
            ([0, 0, 1023], [1024, 1024, 1024])
        );
        // A node finer than a leaf cell: its steps scale up.
        assert_eq!(
            box_of(box_along_z([3, 4], (crate::LEAF_CELL - 1, 1))),
            ([0, 0, 512], [1024, 1024, 1024])
        );
    }

    #[test]
    fn coarse_top_eighths() {
        // Cells of eight leaf cells.
        let cell = LEAF_CELL - 3;
        assert_eq!(top(cell, 16.0), (2, FULL));
        assert_eq!(top(cell, 17.0), (3, FULL / 8));
        assert_eq!(top(cell, 23.0), (3, 7 * FULL / 8));
        assert_eq!(top(cell, -1.0), (0, 7 * FULL / 8));
        // An eighth of a cell of 64 is 8 leaf cells: under it, nothing.
        assert_eq!(top(LEAF_CELL - 6, 7.0), (0, FULL));
        assert_eq!(top(LEAF_CELL - 6, 8.0), (1, FULL / 8));
        // At the leaves, every cell whole.
        assert_eq!(top(LEAF_CELL, 5.0), (5, FULL));
    }
}
