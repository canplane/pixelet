// pxl-engine/ground.rs

//! gpu-portable: the ground's column at a cell, the one rule by which a node's ground is
//! answered (`node`) before what overwrites it. A cell's palette index and height come from
//! the clip level that draws it (`clipmap::ground`). Its cells go by depth, with a cave's air
//! where a cave may take them, by its content's strata and caves (`Resident::Content`).
//!
//! A palette index and a height are stored as a byte and an i16 and read into a word here,
//! since WGSL has no narrower integer.
//!
//! A cell's height from the samples (`height`) is exact, in integers, so the CPU and a GPU
//! draw it alike whatever they fuse or reorder (doc/tip/cpu-and-gpu-agree.md).

use crate::answer::{
    bound_of, bound_word, box_word, eighths, BoxWords, In, Kind, BOX_BITS, BOX_NONE, BOX_STEPS,
    BOX_WHOLE, FULL,
};
use crate::area::{Area, Shard};
use crate::clipmap::Lookup;
use crate::content::Content;
use crate::lattice::TILE_CELLS;
use crate::object::z_start;
use crate::value;
use crate::{
    Zoom, HEIGHT_BITS, LEAF_CELL_ZOOM, LEAF_ZOOM, MIN_ZOOM, SEA_LEVEL, SHARD_ZOOM, UNIT_ZOOM,
};

/// The value space's void and unknown as the words a palette index is read into.
pub const VOID: u32 = value::VOID;
pub const UNKNOWN: u32 = value::UNKNOWN;

/// The inner elevation samples across one leaf tile. A leaf tile is about twice as many
/// meters wide as it has units on a side, and the source's spacing is about 30 m, taken as
/// 32 m.
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

/// The height the surface draws at the center of a tile's cell `(x, y)`, in leaf cells, from
/// its padded elevation samples `ele` (`ele_idx`). It is the one producer of a cell's
/// height, which the core's fill and the bake both run (fill.rs `columns`): Catmull-Rom over
/// the 4 × 4 samples around the center, rounded half to even and held to their range, so a
/// bound over the surface is the samples' own.
///
/// It is exact, its bits counted. A sample is an `i16`. A center lies an odd number of
/// `CELL_STEPS`ths of a sample past one (`window`), where each weight is an integer over
/// `1 << WEIGHT_BITS` (`catmull_rom`), under 2^16 in size. A sample times a weight is under
/// 2^31. A row's sum, with its weights' sizes summing under 1.23 · 2^16, is under 2^31.3. A
/// row times a weight, the four summed, is under 2^47.6 and fits an `i64`, whose high word is
/// the height's floor and whose low word is the fraction to round. Filling a brick costs, for
/// each column, 16 products of 32 bits and 4 of 64.
pub fn height(ele: impl Fn(usize) -> i32, [x, y]: [i32; 2]) -> i32 {
    let (x0, mx) = window(x);
    let (y0, my) = window(y);
    let (wx, wy) = (catmull_rom(mx), catmull_rom(my));
    let (mut sum, mut lo, mut hi) = (0i64, i32::MAX, i32::MIN);
    let mut j = 0;
    while j < 4 {
        let py = clamp_idx(y0 + j);
        let mut row = 0i64;
        let mut i = 0;
        while i < 4 {
            let v = ele(ele_idx(clamp_idx(x0 + i), py));
            lo = lo.min(v);
            hi = hi.max(v);
            row += i64::from(v * wx[i as usize]);
            i += 1;
        }
        sum += row * i64::from(wy[j as usize]);
        j += 1;
    }
    let one = 1i64 << (2 * WEIGHT_BITS);
    let (floor, frac) = (sum >> (2 * WEIGHT_BITS), sum & (one - 1));
    let up = 2 * frac > one || 2 * frac == one && floor & 1 == 1;
    ((floor + i64::from(up)) as i32).clamp(lo, hi)
}

/// What the clip levels bound under a node, each source on its own: the ground's faces,
/// lowest and highest, and the leaf cells the ways fill, lowest and past the highest
/// (`clipmap::BOUND_WORDS`, `fill::mip`).
pub type Bounds = [[i32; 2]; 2];

/// Bounds as their two bound words.
pub(crate) fn bounds_word(b: Bounds) -> [i32; 2] {
    [bound_word(b[0]), bound_word(b[1])]
}

/// The bounds that `bounds_word` packed.
pub fn bounds_of(w: [i32; 2]) -> Bounds {
    [bound_of(w[0]), bound_of(w[1])]
}

/// Two bounds joined, each source's on its own.
pub fn joined(a: Bounds, b: Bounds) -> Bounds {
    [
        [a[0][0].min(b[0][0]), a[0][1].max(b[0][1])],
        [a[1][0].min(b[1][0]), a[1][1].max(b[1][1])],
    ]
}

/// The bits a palette index takes in a cell's word, four to the word.
pub const PALETTE_BITS: u32 = 8;

/// A cell's word's four palette indices, `dx + 2·dy`.
pub fn palette_indices(word: u32) -> [u32; 4] {
    let m = (1 << PALETTE_BITS) - 1;
    [
        word & m,
        word >> PALETTE_BITS & m,
        word >> (2 * PALETTE_BITS) & m,
        word >> (3 * PALETTE_BITS),
    ]
}

/// Four palette indices as a cell's word.
pub fn word([a, b, c, d]: [u32; 4]) -> u32 {
    a | b << PALETTE_BITS | c << (2 * PALETTE_BITS) | d << (3 * PALETTE_BITS)
}

/// A cell's own look at `cell_zoom`: the majority of the four it folds, as the
/// bake folds a tile's (pxl-spec bake/fold.rs `fold_ground`).
pub fn own(w: u32, cell_zoom: Zoom) -> u32 {
    majority(palette_indices(w), cell_zoom)
}

/// The steps of half a cell that an elevation sample spans, since a cell's center lies on a
/// half cell.
pub const CELL_STEPS: i32 = 2 * (TILE_CELLS / TILE_ELE_SPAN) as i32;

/// The bits of a weight's fraction (`catmull_rom`): its denominator is
/// `2 · CELL_STEPS³`.
pub const WEIGHT_BITS: u32 = 1 + 3 * CELL_STEPS.trailing_zeros();

/// The first of the four samples the surface reads along an axis about
/// cell `g`'s center, and how far past the second the center falls, in
/// `CELL_STEPS`ths of a sample, odd: the center `g + ½` lies at
/// `(g + ½) / (CELL_STEPS / 2) + ELE_PAD − ½` samples.
pub fn window(g: i32) -> (i32, i32) {
    let s = 2 * g + 1 + CELL_STEPS * ELE_PAD as i32 - CELL_STEPS / 2;
    ((s >> CELL_STEPS.trailing_zeros()) - 1, s & (CELL_STEPS - 1))
}

/// A sample index on the padded grid: past its edge, the edge sample stands.
pub fn clamp_idx(i: i32) -> u32 {
    i.clamp(0, ELE_PADDED_SPAN as i32 - 1) as u32
}

/// Catmull-Rom's four weights at `m / CELL_STEPS` past the second sample,
/// each times `1 << WEIGHT_BITS`, an integer: `−½t³ + t² − ½t`,
/// `1½t³ − 2½t² + 1`, `−1½t³ + 2t² + ½t` and `½t³ − ½t²` over the
/// denominator.
pub fn catmull_rom(m: i32) -> [i32; 4] {
    let n = CELL_STEPS;
    [
        -m * (n - m) * (n - m),
        3 * m * m * m - 5 * n * m * m + 2 * n * n * n,
        -3 * m * m * m + 4 * n * m * m + n * n * m,
        m * m * (m - n),
    ]
}

const _: () = assert!(
    (CELL_STEPS as u32).is_power_of_two()
        && 2 * CELL_STEPS * CELL_STEPS * CELL_STEPS == 1 << WEIGHT_BITS,
    "a weight's denominator is a power of two"
);
const _: () = assert!(
    2 * WEIGHT_BITS == u32::BITS,
    "a height's sum's high word is its floor"
);

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

/// A height in leaf cells as cells of `cell_zoom` (`top`).
pub fn cells(cell_zoom: Zoom, h: f32) -> i32 {
    top(cell_zoom, h).0
}

/// A height in leaf cells as cells of `cell_zoom`, and the share of the top cell it fills,
/// in `answer::FULL`'s units. Finer than the leaves, the height is shifted and every cell is
/// whole. Coarser, the height is the mean in cells: its top is the cell it ends in where it
/// fills an eighth of that cell or more (`answer::eighths`), keeping the share filled there,
/// and otherwise the cell under it, whole. So a coarse column's ground is the volume its mean
/// height fills, as a cell of it is drawn: whole under its top, with its top a box of its
/// share's volume.
pub fn top(cell_zoom: Zoom, h: f32) -> (i32, u32) {
    let d = cell_zoom as i32 - LEAF_CELL_ZOOM as i32;
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

/// How many cells of `cell_zoom` tall a leaf cell is: one where a cell folds the leaves.
pub fn leaf_cells(cell_zoom: Zoom) -> i32 {
    1 << (cell_zoom as i32 - LEAF_CELL_ZOOM as i32).max(0)
}

/// The face of a top `top` cells of `cell_zoom` high, in leaf cells, as the grid draws it.
/// Finer than the leaves, it is the leaf cell the top's cell lies in, rounded down under sea
/// level as over it, as a cell's index is. Coarser, it is the mean as its cells round it
/// (pxl-render world_rule.wgsl `face_height`).
pub fn face_height(cell_zoom: Zoom, top: i32) -> i32 {
    let d = cell_zoom as i32 - LEAF_CELL_ZOOM as i32;
    if d >= 0 {
        top >> d
    } else {
        top << -d
    }
}

/// The face cell of a column whose face is `face` leaf cells high, in cells of `cell_zoom`:
/// the first cell of the ground under the face, which a cave may take (`Cell::opens`) and in
/// which a way's bore is lined (pxl-engine `way::way_cell`).
pub(crate) fn face_cell(cell_zoom: Zoom, face: i32) -> i32 {
    cells(cell_zoom, face as f32) - leaf_cells(cell_zoom)
}

/// The top of the bedrock under a face `face` leaf cells high, in cells of `cell_zoom`, as
/// `C` lays it (`Content::bedrock`).
pub fn rock<C: Content>(cell_zoom: Zoom, face: i32) -> i32 {
    cells(cell_zoom, C::bedrock(face) as f32)
}

/// Where the ground may fill the node `(zoom, c)` along z, at the zooms from `far` to `near`
/// that a reader asks within it, where the faces under it at those zooms lie between `lo` and
/// `hi` leaf cells (`node`'s relief, all those zooms joined), in leaf cells by the ground's
/// one rule (`cells`, `Cell::opens`). Coarser than a shard, the node may also hold the Earth
/// past the service area, whose face is at sea level.
///
/// The slab's top lies past the highest top any of those zooms rounds `hi` to, since a
/// coarser zoom's cell may round past a finer zoom's. Its bottom is the world's bottom: a node
/// a cave takes all of is split, and its cells say so (`node`). It is nothing (`EMPTY`) where
/// no zoom is asked within the node.
///
/// What fills the node is the part of it inside the slab (`meets`). The node's bottom and top
/// are whole cells of every zoom finer than it, an even number of each, and `cells` ends a
/// height at the cell it fills an eighth of or more (`top`). So a zoom's top is at most the
/// bottom exactly when the face lies under an eighth of that zoom's cell over it, and the
/// finest zoom decides it for all of them. A top kept short of whole still lies in the slab
/// whole, which only splits more.
pub fn slab(zoom: Zoom, [_, hi]: [i32; 2], [far, near]: [Zoom; 2]) -> [i32; 2] {
    let hi = if zoom < SHARD_ZOOM && hi < SEA_LEVEL {
        SEA_LEVEL
    } else {
        hi
    };
    let mut over = EMPTY[1];
    let mut z = coarsest(zoom, far);
    while z <= near {
        let top = face_height(z, cells(z, hi as f32));
        over = if top > over { top } else { over };
        z += 1;
    }
    [UNBOUNDED[0], over]
}

/// The coarsest zoom asked under a node at `zoom` that is coarser than asked, where `far` is
/// the coarsest any ray asks within it. It is never the node's own zoom, since the node is
/// coarser than asked, so it is no coarser than its children's. Every source's bound over a
/// node is read from this zoom to the finest asked.
pub fn coarsest(zoom: Zoom, far: Zoom) -> Zoom {
    if far > zoom + 1 {
        far
    } else {
        zoom + 1
    }
}

/// A leaf cell's index at the world's floor, under sea level.
pub const SEA_CELLS: i32 = 1 << (HEIGHT_BITS - 1);

/// Whether the leaf cells `[lo, hi)` along z meet the node `(zoom, z)`, read down to `near`.
/// Finer than the leaves, a leaf cell is cut 2^e a side, so every height is a whole count of
/// cells.
pub fn meets([lo, hi]: [i32; 2], (zoom, z): (Zoom, i32), near: Zoom) -> bool {
    let e = near.max(LEAF_CELL_ZOOM) - LEAF_CELL_ZOOM;
    let s = LEAF_CELL_ZOOM + e - zoom;
    let first = ((z + z_start(zoom)) << s) - (SEA_CELLS << e);
    first < hi << e && first + (1 << s) > lo << e
}

/// A bound over every height a cell holds, the whole range of an i16. It is a relief: the
/// lowest and highest face heights, both included. It is not a span of what may fill along z,
/// which is half-open (`slab`) and is another fact in another unit.
pub const UNBOUNDED: [i32; 2] = [i16::MIN as i32, i16::MAX as i32];

/// The bound of no ground, lowest over highest: what joins as nothing.
pub const EMPTY: [i32; 2] = [i16::MAX as i32, i16::MIN as i32];

/// The ground resident where a node reads it: the clip levels, as the CPU
/// read them back or a GPU filled them, read the one way (`clipmap::ground`,
/// `clipmap::bound_over`).
pub trait Resident: Area {
    /// The content whose rules its ground follows: its caves and its strata.
    type Content: Content;
    /// The lowest and highest that the ground's cells reach over the node `node`, and the ways'
    /// over them, kept apart (`Bounds`), read from the finest held tile over every zoom from
    /// `cells`' coarsest to its finest (`clipmap::bound_over`). Where no brick it reads is held,
    /// the bounds are no tighter than the heights' whole range (`UNBOUNDED`), which only splits
    /// more; where no tile over the node is held, it returns false. `place` is where the cells
    /// at `cells`' finest zoom of a node over it were found, or `Lookup::NONE`, and becomes where
    /// the node's own are found (`clipmap::step`).
    fn relief(
        &mut self,
        place: &mut Lookup,
        cells: [Zoom; 2],
        node: (Zoom, [i32; 3]),
    ) -> (Bounds, bool);
    /// The column of the cell `c` at `zoom` from the clip levels (`clipmap::ground`), or the
    /// wall where it is the wall (`area::is_wall`, read from `shard`, what is known of the area
    /// there). Its cell is taken where `place` found the zoom's cells (`relief`), or found anew
    /// where `place` is `NONE`.
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> Cell;
}

/// What the ground answers of a node (`node`): its kind, and a cell's
/// asset and palette index.
pub type Answer = In<[u32; 2]>;

/// The ground's answer for a node: the one way a descent asks the ground. `is_cell` says
/// whether the node is asked at its own zoom. The caves are asked of the node itself only
/// where they may take it (`Content::hollow`, a share), and the strata are the content `H`'s
/// (`Cell::asset`). Past the service area the answer is the Earth's. Otherwise the node's own
/// bound comes first, from each zoom's tile its cells are read from (`Resident::relief`). At
/// its own zoom, the node is air above the ground, and otherwise its column's asset
/// (`Cell::asset`) with its palette index. A coarser node no part of which lies where the
/// ground may fill at the zooms asked (`slab`, `meets`) is a cube of air its size, and
/// otherwise it is split. Where no tile over it is held, a node at its own zoom is the lack,
/// and a coarser node is split, since it is not known to be empty.
///
/// `shard` is what is known of the area at the node, known at a shard or finer; it is the
/// asker's, which asks it once at its entry (`Shard::at`). `place` is where the cells of a
/// node over it were held (`Resident::relief`), or `NONE` where none were, and becomes the
/// node's: where a cell's column was read, at which the ways written over it are read
/// (pxl-engine `world::node`), and `NONE` past the area, where nothing is read. With the
/// answer it returns the column the cell was read from, `Cell::NONE` for a coarser node, and
/// the ways' bound read with the ground's own (`Resident::relief`), `EMPTY` where none was.
pub fn node<H: Resident>(
    held: &mut H,
    shard: Shard,
    place: &mut Lookup,
    (zoom, c): (Zoom, [i32; 3]),
    [far, near]: [Zoom; 2],
    is_cell: bool,
) -> (Answer, Cell, [i32; 2]) {
    let air = In {
        kind: Kind::Cell,
        asset: [VOID, 0],
    };
    // A split whose span no bound tells may hold anything anywhere in it.
    let split = In {
        kind: Kind::Split,
        asset: BOX_WHOLE,
    };
    let lacks = In {
        kind: Kind::Lacks,
        asset: [VOID, 0],
    };
    // Past the area: a cell's own Earth, or a node's where it lies all above sea level, which
    // no shard does, since a shard spans the world's height around sea level. No node of a
    // shard's size or finer straddles the area's edge.
    if shard.past() {
        let earth = H::Content::earth(c[2]);
        let answer = if is_cell {
            In {
                kind: Kind::Cell,
                asset: [earth, 0],
            }
        } else if earth == VOID && zoom > SHARD_ZOOM {
            air
        } else {
            split
        };
        *place = Lookup::NONE;
        return (answer, Cell::NONE, EMPTY);
    }
    // A node coarser than any tile lies in no one tile's index, so it is split.
    if zoom < MIN_ZOOM {
        return (split, Cell::NONE, EMPTY);
    }
    // A cell's columns are its zoom's. A coarser node's are those of every zoom asked within it,
    // from the coarsest to `near`. Each zoom's cells come from its own tile, whose face, its own
    // samples' bicubic, no other zoom's bounds bound from above or below, so the bound is all of
    // them joined, read once a clip level (`Resident::relief`).
    let first = coarsest(zoom, far);
    let cells = if is_cell {
        [zoom, zoom]
    } else {
        [if first > near { near } else { first }, near]
    };
    let ([relief, ways], ok) = held.relief(place, cells, (zoom, c));
    // Not held at a zoom asked: a cell is the lack. A coarser node is not known to be empty, so
    // it is split, and a ray stops only at a cell it meets that is not held.
    if !ok {
        return (if is_cell { lacks } else { split }, Cell::NONE, EMPTY);
    }
    if is_cell {
        let column = held.cell(shard, *place, zoom, c);
        // The caves' share where they may take the cell, beside its asset:
        // the cell is air where they take it all.
        let ground = column.asset::<H::Content>(zoom, c[2]);
        let cave = if ground != VOID && column.opens::<H::Content>(zoom, c[2]) {
            H::Content::hollow((zoom, c))
        } else {
            0
        };
        let asset = if cave >= FULL { VOID } else { ground };
        return (
            In {
                kind: Kind::Cell,
                asset: [asset, cave],
            },
            column,
            ways,
        );
    }
    // Air where none of the node lies where the ground may fill, or else a split with its box,
    // the span along z the ground may fill, in the node's steps. A node a cave takes all of
    // still splits, and its cells say so.
    let s = slab(zoom, relief, [far, near]);
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

/// A cell's ground, as a column reads it: its palette index (`UNKNOWN` for the service
/// area's wall), its top in cells of its zoom and the share of its top cell it fills (`top`),
/// and the four palette indices its fold took (`palette_indices`, `dx + 2·dy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub palette_idx: u32,
    pub top: i32,
    pub part: u32,
    pub fold: [u32; 4],
}

impl Cell {
    /// No ground: void at sea level, as a column no tile is read for.
    pub const NONE: Cell = Cell {
        palette_idx: VOID,
        top: 0,
        part: FULL,
        fold: [VOID; 4],
    };

    /// The share of cell `z` that its ground fills, in `answer::FULL`'s units, where a cell of
    /// it fills there (`asset`): whole under its top cell, and its part in the top cell (`top`).
    pub fn cover(&self, z: i32) -> u32 {
        if z == self.top - 1 {
            self.part
        } else {
            FULL
        }
    }

    /// Whether it has ground at all: a palette index the catalog grounds, or the wall.
    pub fn is_grounded(&self) -> bool {
        self.palette_idx == UNKNOWN || value::grounds(self.palette_idx)
    }

    /// Whether a cave may take cell `z` of the column at `cell_zoom`, by the ground's one rule:
    /// ground under the face cell, above the bedrock and off the area's wall, with the face and
    /// the bedrock rounded to cells as the column rounds them. What lies under a top kept short
    /// of whole is ground, as under a shrunk box, so a cave does not take the cell under it
    /// either.
    pub fn opens<C: Content>(&self, cell_zoom: Zoom, z: i32) -> bool {
        let face = face_height(cell_zoom, self.top);
        let under = face_cell(cell_zoom, face) - (self.part < FULL) as i32;
        self.palette_idx != UNKNOWN && z < under && z >= rock::<C>(cell_zoom, face)
    }

    /// The four palette indices that asset `g` of the column at `cell_zoom` is drawn by. Where
    /// `g` is the cell's own look at a cell coarser than the leaves', they are the four its fold
    /// took, whose mean it is drawn as, as the cell reads from afar. Otherwise, for a stratum
    /// under it or a leaf's cell, they are `g` four times. A way written over the cell gives its
    /// own palette index four times (`world::node`).
    pub fn four(&self, cell_zoom: Zoom, g: u32) -> [u32; 4] {
        if cell_zoom >= LEAF_CELL_ZOOM || g != self.palette_idx {
            [g; 4]
        } else {
            self.fold
        }
    }

    /// The asset in cell `z` of the column at `cell_zoom`, before the caves take their share
    /// (`opens`, `Content::hollow`), by `C`'s strata (`Content::by_depth`): the ground's palette
    /// index, which at a coarser cell is its fold's majority, as what is placed is its fold's, or
    /// `VOID` for air.
    pub fn asset<C: Content>(&self, cell_zoom: Zoom, z: i32) -> u32 {
        if !self.is_grounded() {
            return VOID;
        }
        C::by_depth(cell_zoom, self.palette_idx, self.top, z)
    }
}

const _: () = assert!(
    LEAF_CELL_ZOOM == UNIT_ZOOM,
    "a span's leaf cell is the world's unit"
);
const _: () = assert!(
    -(1i32 << (HEIGHT_BITS - 1)) == i16::MIN as i32
        && (1i32 << (HEIGHT_BITS - 1)) - 1 == i16::MAX as i32,
    "the world's z, in leaf cells, is i16's range"
);

/// The leaf cells `[lo, hi)` along z of the node `(zoom, z)`, as its box: the node's steps
/// along z that they meet, rounded out, with the node whole across. A node finer than a leaf
/// cell is held whole; an empty span, or one the node does not meet, gives `BOX_NONE`.
pub(crate) fn box_along_z([lo, hi]: [i32; 2], (zoom, z): (Zoom, i32)) -> BoxWords {
    if lo >= hi {
        return BOX_NONE;
    }
    if zoom >= LEAF_CELL_ZOOM {
        return BOX_WHOLE;
    }
    // The step of the node a leaf cell `v` lies in, as the node's own
    // steps count it: `v` scaled by `BOX_STEPS` / side, less the node's first.
    let shift = LEAF_CELL_ZOOM - zoom;
    let step = |v: i32| -> i32 {
        let (v, z) = (v + SEA_CELLS, z + z_start(zoom));
        if shift >= BOX_BITS {
            (v >> (shift - BOX_BITS)) - (z << BOX_BITS)
        } else {
            (v << (BOX_BITS - shift)) - (z << BOX_BITS)
        }
    };
    // The span's first step and the step past its last, as the node counts them. The end lies
    // past the step that the span's last leaf cell lies in, since a leaf cell a step wide or
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
        let zoom = LEAF_CELL_ZOOM - 6;
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
        // A root node, with the world's floor at its own: its middle steps hold the span.
        assert_eq!(
            box_of(box_along_z([-5, 7], (0, 0))),
            ([0, 0, 1], [1024, 1024, 3])
        );
        // A node finer than a leaf cell: its steps scale up.
        assert_eq!(
            box_of(box_along_z([3, 4], (LEAF_CELL_ZOOM - 1, 1))),
            ([0, 0, 512], [1024, 1024, 1024])
        );
    }

    #[test]
    fn coarse_top_eighths() {
        // Cells of eight leaf cells.
        let cell_zoom = LEAF_CELL_ZOOM - 3;
        assert_eq!(top(cell_zoom, 16.0), (2, FULL));
        assert_eq!(top(cell_zoom, 17.0), (3, FULL / 8));
        assert_eq!(top(cell_zoom, 23.0), (3, 7 * FULL / 8));
        assert_eq!(top(cell_zoom, -1.0), (0, 7 * FULL / 8));
        // An eighth of a cell of 64 is 8 leaf cells: under it, nothing.
        assert_eq!(top(LEAF_CELL_ZOOM - 6, 7.0), (0, FULL));
        assert_eq!(top(LEAF_CELL_ZOOM - 6, 8.0), (1, FULL / 8));
        // At the leaves, every cell whole.
        assert_eq!(top(LEAF_CELL_ZOOM, 5.0), (5, FULL));
    }

    /// A face past the leaves is the leaf cell its top's cell lies in,
    /// rounded down under sea level as over it; under the leaves, the cells
    /// scaled.
    #[test]
    fn face_height_floors() {
        let cell_zoom = LEAF_CELL_ZOOM + 2;
        assert_eq!(face_height(cell_zoom, 8), 2);
        assert_eq!(face_height(cell_zoom, 9), 2);
        assert_eq!(face_height(cell_zoom, -8), -2);
        assert_eq!(face_height(cell_zoom, -9), -3);
        assert_eq!(face_height(cell_zoom, -1), -1);
        assert_eq!(face_height(LEAF_CELL_ZOOM - 3, -5), -40);
    }
}
