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

use crate::area::{earth, past, tile_at, walled, Area};
use crate::cast::{In, Kind};
use crate::caves::Cave;
use crate::cube::{ground, sentinel, Ground};
use crate::lattice::EXTENT;
use crate::lattice::GROUND;
pub use crate::lattice::TILE_CELLS;
use crate::{Zoom, LEAF_CELL, LEAF_ZOOM, SHARD_ZOOM, UNIT_ZOOM};

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

/// Cells on a tile's side in its index lane (pxl-spec bake/tile/terrain.rs
/// `Index`): one per elevation sample spacing, at 4 KiB a tile.
pub const EXTENT_SPAN: u32 = TILE_ELE_SPAN;
const _: () = assert!(EXTENT_SPAN.is_power_of_two(), "an index cell is a block of the lattice, a power of two of the unit");

/// The index lane's cells' zoom on a leaf tile.
pub const EXTENT_CELL: Zoom = LEAF_ZOOM + EXTENT_SPAN.trailing_zeros() as Zoom;

/// Cells in a tile's index lane: its `EXTENT` cells, each the lowest and the
/// highest the ground draws over it; its coarser levels follow them
/// (`EXTENT.coarse_index`).
pub const EXTENT_LANE: u32 = EXTENT.tile_cells() * EXTENT.tile_cells();

/// How far past the lane's facts the client draws a cell, down and up: the
/// way's own cell under a bore's or a cutting's surface. Every read of the
/// lane widens by it.
pub const DRAWN_PAST: [i32; 2] = [-1, 0];

/// What is drawn over the cell of `across` × `across` index cells at
/// `(x, y)` of a tile's index lane, widened by `DRAWN_PAST`: one read at any
/// size, a coarser level holding the join of the cells under it. `lane`
/// reads the lane's cell `i`, its coarser levels from `EXTENT_LANE`; `across`
/// a power of two and `(x, y)` a multiple of it.
pub fn over(lane: impl Fn(u32) -> [i32; 2], [x, y]: [u32; 2], across: u32) -> [i32; 2] {
    let j = across.trailing_zeros();
    let [lo, hi] = lane(if j == 0 { EXTENT.cell_index(x, y) } else { EXTENT_LANE + EXTENT.coarse_index(j, x, y) });
    [lo + DRAWN_PAST[0], hi + DRAWN_PAST[1]]
}

/// The lowest and highest the ground draws over the column of the node
/// `(zoom, c)`, from the index lane `lane` of the tile at `from` it lies in.
pub fn relief(lane: impl Fn(u32) -> [i32; 2], from: Zoom, (zoom, c): (Zoom, [i32; 2])) -> [i32; 2] {
    let index = EXTENT.cell_zoom(from);
    let (first, across) = if zoom < index {
        ([c[0] << (index - zoom), c[1] << (index - zoom)], 1 << (index - zoom))
    } else {
        ([c[0] >> (zoom - index), c[1] >> (zoom - index)], 1)
    };
    let n = EXTENT.tile_cells() as i32;
    over(lane, [(first[0] & (n - 1)) as u32, (first[1] & (n - 1)) as u32], across)
}

/// A layer of ground cells under a face, in leaf cells, before the stone.
const FILLER_DEPTH: i32 = 3;
/// Sandstone under a sand or beach face's filler, in leaf cells.
const SANDSTONE_DEPTH: i32 = 2;
/// The bedrock's top under a face, this far under it or under sea level,
/// whichever is lower, in leaf cells (pxl-spec block/limit.rs `floor`).
pub const BEDROCK_DEPTH: i32 = 64;

/// One tile as held, its cells read in place: a slice on the CPU, a frame
/// of a storage buffer on the GPU.
pub trait Tile {
    /// The look of the tile's cell `(x, y)`, `0..TILE_CELLS` each.
    fn look(&self, x: u32, y: u32) -> u32;
    /// The height of the tile's cell `(x, y)`, in leaf cells (`height`).
    fn height(&self, x: u32, y: u32) -> i32;
}

/// A cell `v` of a tile `n` × `n` times finer than `tile`, counted from
/// `tile`'s first cell at that finer size: the look and height of `tile`'s
/// cell holding it, so a coarser tile standing in draws its own cells and
/// invents none finer; off `tile`, which knows nothing there, `void` at sea
/// level.
pub fn read(tile: &impl Tile, v: [i32; 2], n: u32) -> (u32, i32) {
    if !holds(v, n) {
        return (VOID, 0);
    }
    let s = n.trailing_zeros();
    let (x, y) = ((v[0] >> s) as u32, (v[1] >> s) as u32);
    (tile.look(x, y), tile.height(x, y))
}

/// The height the surface draws at the center of a tile's cell `(x, y)`,
/// in leaf cells, off its padded elevation samples `ele` (`ele_index`): the
/// one producer of a cell's height, which the CPU computes where a cell is
/// read and a GPU once a held tile, into its grid.
pub fn height(ele: impl Fn(usize) -> i32, [x, y]: [u32; 2]) -> i32 {
    bicubic(ele, x as f32 + 0.5, y as f32 + 0.5)
}

/// The levels of a held tile's bound over its cells' heights, which follow
/// its cells in a GPU's grid: each entry of level `j` the lowest and highest
/// of the 2^j × 2^j cells under it, the join of four of level j − 1's
/// (`join`), up to the tile whole. A bound read off it is the grid's own
/// reads, so a cell's height, a fold's mean of them and a coarser tile's
/// cell standing in all lie in it.
pub const BOUND_LEVELS: u32 = TILE_CELLS.trailing_zeros();

/// Where level `j` of a tile's grid begins, in words: level 0 its cells,
/// then each level's entries row by row, level `i` (TILE_CELLS / 2^i)² of
/// them, so the levels before `j` are (4·n − 4·n / 4^j) / 3, n the cells.
pub const fn bound_at(j: u32) -> u32 {
    let n = 4 * TILE_CELLS * TILE_CELLS;
    (n - (n >> (2 * j))) / 3
}

/// Words a tile's grid holds: its cells' heights, then its bound's levels.
pub const GRID_WORDS: u32 = bound_at(BOUND_LEVELS + 1);

/// An entry of a bound as a word, its lowest in the low half and its
/// highest in the high: each a cell's height, which `bicubic` holds to its
/// i16 samples.
pub fn bound_word([lo, hi]: [i32; 2]) -> i32 {
    lo & 0xffff | hi << 16
}

/// `bound_word`'s lowest and highest.
pub fn bound_of(w: i32) -> [i32; 2] {
    [w << 16 >> 16, w >> 16]
}

/// Entry `(x, y)` of level `j` of a tile's bound off its grid, `grid` its
/// word `i`: the join of the four under it, at level 1 four cells' heights.
pub fn join(grid: impl Fn(u32) -> i32, j: u32, [x, y]: [u32; 2]) -> i32 {
    let side = TILE_CELLS >> (j - 1);
    let under = |q: u32| {
        let w = grid(bound_at(j - 1) + (2 * y + q / 2) * side + 2 * x + q % 2);
        if j == 1 {
            [w, w]
        } else {
            bound_of(w)
        }
    };
    let [mut lo, mut hi] = under(0);
    let mut q = 1;
    while q < 4 {
        let [l, h] = under(q);
        (lo, hi) = (lo.min(l), hi.max(h));
        q += 1;
    }
    bound_word([lo, hi])
}

/// The lowest and highest the cells of the tile at `from` reach under the
/// node `(zoom, c)` it holds, `grid` its grid's word `i`: one entry, of the
/// level whose entries are the node's columns, or the cell holding a node
/// finer than a cell.
pub fn bounded(grid: impl Fn(u32) -> i32, from: Zoom, (zoom, c): (Zoom, [i32; 2])) -> [i32; 2] {
    let cell = GROUND.cell_zoom(from);
    let within = |side: u32, v: i32| (v & (side as i32 - 1)) as u32;
    if zoom >= cell {
        let s = zoom - cell;
        let h = grid(within(TILE_CELLS, c[1] >> s) * TILE_CELLS + within(TILE_CELLS, c[0] >> s));
        return [h, h];
    }
    let j = cell - zoom;
    let side = TILE_CELLS >> j;
    bound_of(grid(bound_at(j) + within(side, c[1]) * side + within(side, c[0])))
}

/// Whether `tile` holds the cell `v` of a tile `n` × `n` times finer, `n` a
/// power of two, so a cell's coarser one is its index shifted, floored.
fn holds(v: [i32; 2], n: u32) -> bool {
    let s = n.trailing_zeros();
    let (x, y) = (v[0] >> s, v[1] >> s);
    let cells = TILE_CELLS as i32;
    x >= 0 && y >= 0 && x < cells && y < cells
}

/// `read`'s look alone.
pub fn look(tile: &impl Tile, v: [i32; 2], n: u32) -> u32 {
    if !holds(v, n) {
        return VOID;
    }
    let s = n.trailing_zeros();
    tile.look((v[0] >> s) as u32, (v[1] >> s) as u32)
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
pub fn empty(area: &impl Area, (zoom, c): (Zoom, [i32; 3]), [lo, hi]: [i32; 2], [far, near]: [Zoom; 2], hollow: bool) -> bool {
    let [lo, hi] = if zoom < SHARD_ZOOM { [lo.min(0), hi.max(0)] } else { [lo, hi] };
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
    let rock = (hi.min(0) - BEDROCK_DEPTH) << e;
    hollow && under && rock <= bottom + half(near) && !walled(area, near, (zoom, [c[0], c[1]]))
}

/// Whether the block `z` of a cell at `cell` lies over the ground wherever
/// in the column the faces under it reach at most `hi` leaf cells: a cell's
/// height is `bicubic`'s, held to the samples it reads, or a fold's mean of
/// them, and `hi` bounds those samples (pxl-spec bake/surface.rs `bound`),
/// so its top, rounded as the grid rounds it, is at most `hi`'s.
pub fn above(cell: Zoom, z: i32, hi: i32) -> bool {
    z >= blocks(cell, hi as f32)
}

/// A bound over every height a cell holds, i16's whole range (`bicubic`).
pub const UNBOUNDED: [i32; 2] = [i16::MIN as i32, i16::MAX as i32];

/// How the ground is held where a node reads it: the CPU's residency, a
/// GPU's window over the tiles it was handed.
pub trait Held: Area {
    /// The lowest and highest the ground's cells at the zooms `cells`, from
    /// the coarsest to the finest asked, reach over the node `(zoom, c)`,
    /// each off the finest held tile it is read from; a bound no tighter
    /// than the heights' whole range (`UNBOUNDED`) where the source cannot
    /// bound them, which only splits more; false where no tile over it is
    /// held.
    fn relief(&mut self, cells: [Zoom; 2], node: (Zoom, [i32; 3])) -> ([i32; 2], bool);
    /// The column of the block `c` at `zoom` as the grid reads it, the
    /// service area's edge laid on it (`Cell`); false where it cannot be read.
    fn cell(&mut self, zoom: Zoom, c: [i32; 3]) -> (Cell, bool);
}

/// The ground's answer for a node, the one way a descent asks the ground,
/// `cave` the caves there and `block` whether it is asked at its own zoom:
/// past the service area, the Earth's; else the node's own bound first, the
/// index lane of the tile its cells are read from — a block over it is air,
/// else its cell's block (`Cell::block`), its look and its fold's four; a
/// coarser node nothing under fills at the zooms asked (`empty`) a block of
/// air its size, else split; where no tile over it is held, the lack.
pub fn node(held: &mut impl Held, cave: Cave, (zoom, c): (Zoom, [i32; 3]), [far, near]: [Zoom; 2], block: bool) -> In<(u32, [u32; 4])> {
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
    if zoom >= SHARD_ZOOM && past(held, tile_at(zoom, [c[0], c[1]])) {
        let earth = earth(c[2]);
        return if block {
            In {
                kind: Kind::Block,
                block: (earth, [earth; 4]),
            }
        } else if earth == VOID {
            air
        } else {
            split
        };
    }
    // Coarser than any tile, no one index holds it.
    if zoom < GROUND.top() {
        return split;
    }
    // A block's cells are its zoom's; a coarser node's, those of each zoom
    // asked within it (`empty`).
    let cells = if block { [zoom, zoom] } else { [if far > zoom + 1 { far } else { zoom + 1 }, near] };
    let (relief, ok) = held.relief(cells, (zoom, c));
    if !ok {
        return lacks;
    }
    if block {
        if above(zoom, c[2], relief[1]) {
            return air;
        }
        let (column, ok) = held.cell(zoom, c);
        if !ok {
            return lacks;
        }
        return In {
            kind: Kind::Block,
            block: column.block(zoom, c[2], cave.hollow()),
        };
    }
    if empty(held, (zoom, c), relief, [far, near], cave.whole(zoom) == (true, true)) {
        air
    } else {
        split
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
