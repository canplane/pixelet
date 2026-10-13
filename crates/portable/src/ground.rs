// pxl-portable/ground.rs

//! gpu-portable: the ground's grid at a cell, the one rule pxl-spec
//! block/sample.rs `at` composes before what overwrites it: a cell's look and
//! height off the tile drawing it, their fold a zoom coarser, and its blocks
//! by depth with a cave's air where a cave may take them. Separation (the
//! ways) and what is placed are not here yet: they overwrite the ground on
//! the CPU alone.
//!
//! A look and a height are stored as a byte and an i16 and read into a word
//! here, since WGSL has no narrower integer.
//!
//! Heights are f32 here, as WGSL has them. WGSL's f32 is not strictly IEEE (a
//! division's last place, a contraction into a fused multiply-add), so a
//! height a CPU and a GPU compute from this one source can differ in the
//! last bit where it rounds to a block at a half.

use crate::cube::{ground, sentinel, Ground};
use crate::{Zoom, LEAF_CELL, LEAF_ZOOM, UNIT_ZOOM};

// The catalog's looks this rule names, read into a word.
const VOID: u32 = sentinel::void as u32;
const UNKNOWN: u32 = sentinel::unknown as u32;
const SAND: u32 = ground::sand as u32;
const BEACH: u32 = ground::beach as u32;
const WATER: u32 = ground::water as u32;
const SOIL: u32 = ground::soil as u32;
const SANDSTONE: u32 = ground::sandstone as u32;
const STONE: u32 = ground::stone as u32;

/// Cells a tile is on a side, at every zoom.
pub const TILE_CELLS: u32 = 1 << (LEAF_CELL - LEAF_ZOOM);

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
/// whichever is lower, in leaf cells (pxl-spec block/limit.rs `floor`).
pub const BEDROCK_DEPTH: i32 = 64;

/// One tile as stored, read in place: its cells' looks and its padded
/// elevation samples, a slice on the CPU and a frame of a storage buffer on
/// the GPU.
pub trait Tile {
    /// The look of the tile's cell `(x, y)`, `0..TILE_CELLS` each.
    fn look(&self, x: u32, y: u32) -> u32;
    /// Elevation sample `i` (`ele_index`), in leaf cells.
    fn ele(&self, i: usize) -> i32;
}

/// A cell `v` of a tile `n` × `n` times finer than `tile`, counted from
/// `tile`'s first cell at that finer size: the look of `tile`'s cell holding
/// it and the height the surface draws at its center, in leaf cells, so a
/// coarser tile standing in invents nothing; off `tile`, which knows nothing
/// there, `void` at sea level.
pub fn read(tile: &impl Tile, v: [i32; 2], n: u32) -> (u32, i32) {
    let n = n as i32;
    let (x, y) = (v[0].div_euclid(n), v[1].div_euclid(n));
    let cells = TILE_CELLS as i32;
    if x < 0 || y < 0 || x >= cells || y >= cells {
        return (VOID, 0);
    }
    let at = |v: i32| (v as f32 + 0.5) / n as f32;
    (tile.look(x as u32, y as u32), bicubic(|i| tile.ele(i), at(v[0]), at(v[1])))
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
    let h = if count > 0 { sum as f32 / count as f32 } else { 0.0 };
    (majority(looks, cell), h, looks)
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
        top / (1 << d)
    } else {
        top << -d
    }
}

/// The bedrock's top under a face at `face` leaf cells, in blocks of `cell`.
pub fn rock(cell: Zoom, face: i32) -> i32 {
    blocks(cell, (face.min(0) - BEDROCK_DEPTH) as f32)
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
    /// Whether it has ground at all: a look the catalog grounds, or the wall.
    pub fn grounded(&self) -> bool {
        self.look == UNKNOWN || Ground::grounds(self.look)
    }

    /// Block `z` of the cell at `cell`, a cave's air where `hollow` and a
    /// cave may take it: the ground's look and its fold's four, `void` for
    /// air.
    pub fn block(&self, cell: Zoom, z: i32, hollow: bool) -> (u32, [u32; 4]) {
        let air = (VOID, [VOID; 4]);
        if !self.grounded() {
            return air;
        }
        let g = by_depth(cell, self.look, self.top, z);
        let face = face_height(cell, self.top);
        if g == VOID || hollow && lay(cell, [face, face], self.look == UNKNOWN, [z, z + 1]) == Lay::Open {
            return air;
        }
        let folds = cell >= LEAF_CELL || g != self.look;
        (g, if folds { [g; 4] } else { self.fold })
    }
}
