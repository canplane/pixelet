// pxl-spec/space/pyramid.rs

//! The pyramid: the space hierarchy (indexer.rs) repeated over coarser
//! blocks. A level-k cell is its lattice's first cell k levels up
//! (`Lattice::scale`), 2^k of them on a side, and a level-k page is a page's
//! worth of those cells, 2^k leaf pages on a side, so every extent the
//! hierarchy gives a leaf is the same extent at level k, scaled
//! (issue/20260921-level-step-and-zoom-are-one-number.md).
//!
//! It is the one hierarchy the ground's layers are paged in — terrain and
//! places; the placed objects keep their own trees, on the same geometry
//! (space/README.md). Where a cell is, which page it belongs to, which pages
//! fold into which, and where a cell's center stands are decided here and
//! nowhere else; a layer owns only what a cell carries and how a parent's
//! cell is made from its children's (`Folds`).

use super::hierarchy::{ancestor_of, child_of, first_descendant_of, parent_of, slot_of, Scale};
use super::indexer::{L1, L2, L3, L3_IDX, L4};
use super::pvf3::core::FRAC_W;
use super::pvf3::core::IVEC_X_W;
use super::pvf3::ivec::{self, IVec};
use super::pvf3::projection;
use super::pvf3::pvf3::{self, Pvf3};
use super::pvf3::REACH;
use super::v3::Aabb;
use super::xy::TO_XY;
use crate::bake::constant::{CELL, DECK_CELL, EXTENT_CELL};

/// The coarsest level the pyramid is folded to. At level 10 — zoom 5 — a
/// page spans about 9° of latitude, across which the page-constant vertical
/// scale is already 12% off: past it Web Mercator on a plane stops being
/// honest, and a coarser view is a different projection over the same tiles
/// rather than another level. A bake folds up to it and a server answers
/// nothing above it.
pub const TOP_LEVEL: u32 = 10;

/// The zoom the ground's finest pages are named at, in the 512-pixel tile
/// convention: the X lane is one EPSG:3857 world, so a level has
/// 2^(IVEC_X_W − L3 − level) pages across it, which is a zoom's tile count.
/// Their cells are the standard 256-pixel z16 pixel exactly, so level k is
/// z(16 − k) — its cells z(16 − k)'s pixels — and its pages are named at
/// PAGE_ZOOM − k.
pub const PAGE_ZOOM: u32 = IVEC_X_W - L3.shamt() as u32;

// ---- the lattice's scale ----
//
// A cell is not a unit. A lattice is scales against the unit, as
// sparse_tree's layout is: its first level's cell is one scale, and a level-k cell
// k above it. A page and a chunk are fixed in units — the L3 and L2 blocks —
// so every layer shares the pyramid's pages, levels and names, and how many
// cells stand across one is what its lattice decides. A level is a count
// from the lattice's cell, and the lattice is what turns it into a size.

/// A layer's lattice: its cell's side at the pyramid's first level, z16's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lattice {
    cell: Scale,
}

impl Lattice {
    pub const fn new(cell: Scale) -> Lattice {
        if cell.shamt() + FRAC_W as i32 <= 0 || cell.shamt() > L2.shamt() {
            panic!()
        }
        Lattice { cell }
    }

    /// A level-`level` cell's side.
    pub const fn scale(self, level: u32) -> Scale {
        self.cell.up(level as i32)
    }

    /// Cells on a page's side, at every level, as a shift.
    const fn page_cell_shamt(self) -> u32 {
        L3.above(self.cell) as u32
    }

    /// Cells on a page's side, at every level.
    pub const fn page_cells(self) -> u32 {
        self.cell.count_in(L3)
    }

    /// Cells on a chunk's side, at every level: the L2 block's.
    pub const fn chunk_cells(self) -> u32 {
        self.cell.count_in(L2)
    }

    /// A cell's index in its page: `ty · page_cells + tx`.
    pub const fn cell_index(self, tx: u32, ty: u32) -> u32 {
        ty * self.page_cells() + tx
    }

    /// The level-`level` cell a point stands in. A point and not a unit
    /// address, since a cell may be finer than a unit.
    pub fn cell_of(self, level: u32, at: &Pvf3) -> Cell {
        let page = LevelPage::over(level, L3_IDX(at.i));
        let [x, y, _] = self.scale(level).index_of(at);
        let [x0, y0] = first_descendant_of([ivec::X(page.page), ivec::Y(page.page)], self.page_cell_shamt());
        Cell {
            lattice: self,
            page,
            index: self.cell_index((x - x0) as u32, (y - y0) as u32),
        }
    }

    /// The cell at `index` of `page`.
    pub fn cell_at(self, page: LevelPage, index: u32) -> Cell {
        xpute_core::ensure!(index < self.page_cells() * self.page_cells(), ENOTRECOVERABLE);
        Cell { lattice: self, page, index }
    }

    /// The point at the south-west corner of the cell at (tx, ty) in `page`.
    pub fn cell_base(self, page: LevelPage, tx: u32, ty: u32) -> Pvf3 {
        let [x0, y0] = first_descendant_of([ivec::X(page.page), ivec::Y(page.page)], self.page_cell_shamt());
        self.scale(page.level).base_of([x0 + i64::from(tx), y0 + i64::from(ty), 0])
    }

    // A page's four children lie side by side as one block of 2n × 2n cells
    // a level down, the child page in slot q its (q & 1, q >> 1) square of
    // it. A cell and the one it folds into are parent and child in that
    // block, as anything a level apart is (hierarchy.rs); a page's slot is
    // its square's.

    /// The parent cell a child's cell folds into, the child page in slot `q`.
    pub fn parent_cell(self, q: usize, index: u32) -> u32 {
        let n = self.page_cells() as i64;
        let (page, [tx, ty]) = (child_of([0, 0], q), self.in_page(index).map(i64::from));
        let [x, y] = parent_of([page[0] * n + tx, page[1] * n + ty]);
        self.cell_index(x as u32, y as u32)
    }

    /// The child page's slot and the first of the 2×2 child cells a parent's
    /// cell is folded from: `parent_cell`'s inverse.
    pub fn child_block(self, index: u32) -> (usize, u32) {
        let n = self.page_cells() as i64;
        let [x, y] = child_of(self.in_page(index).map(i64::from), 0);
        (slot_of([x / n, y / n]), self.cell_index((x % n) as u32, (y % n) as u32))
    }

    /// The cells of this lattice that lie under `outer`'s cell at `index` of
    /// the same page, row by row: a block n a side where `outer`'s cell is n
    /// of this one's.
    pub fn under(self, outer: Lattice, index: u32) -> impl Iterator<Item = u32> {
        let n = self.cell.count_in(outer.cell);
        let [x0, y0] = outer.in_page(index).map(|v| v * n);
        (0..n).flat_map(move |dy| (0..n).map(move |dx| self.cell_index(x0 + dx, y0 + dy)))
    }

    /// A cell index as its column and row in its page: `cell_index`'s
    /// inverse.
    pub const fn in_page(self, index: u32) -> [u32; 2] {
        let n = self.page_cells();
        [index % n, index / n]
    }

    // A page's aligned square blocks of cells, 2^j a side for j from 1 to
    // the page's own, lie one size after another from the smallest, each
    // size's row by row: what a summary over any node of the hierarchy a
    // page holds is read from in one place.

    /// A page's blocks, every size together.
    pub const fn page_blocks(self) -> u32 {
        self.first_block(self.page_cell_shamt() + 1)
    }

    /// Where the block 2^`j` cells a side that holds cell (tx, ty) stands
    /// among a page's blocks.
    pub const fn block_index(self, j: u32, tx: u32, ty: u32) -> u32 {
        if j == 0 || j > self.page_cell_shamt() {
            panic!()
        }
        self.first_block(j) + (ty >> j) * (self.page_cells() >> j) + (tx >> j)
    }

    /// Where the blocks 2^`j` cells a side begin: past every smaller size.
    const fn first_block(self, j: u32) -> u32 {
        let mut first = 0;
        let mut k = 1;
        while k < j {
            let side = self.page_cells() >> k;
            first += side * side;
            k += 1;
        }
        first
    }
}

/// The ground's lattice: bake/constant.rs's cell, which terrain and objects
/// are cut in.
pub const GROUND: Lattice = Lattice::new(CELL);

/// The index lane's lattice (bake/format.rs `Index`): a cell of it bounds
/// the heights of every page under it, a block of the ground's cells a side.
pub const EXTENT: Lattice = Lattice::new(EXTENT_CELL);

/// The deck lane's lattice (bake/deck.rs): the ground's cell twice over.
pub const DECK: Lattice = Lattice::new(DECK_CELL);

/// Cells on a page's side in the ground's lattice.
pub const PAGE_CELLS: u32 = GROUND.page_cells();

/// Cells on a chunk's side in the ground's lattice.
pub const CHUNK_CELLS: u32 = GROUND.chunk_cells();

/// Chunks on a page's side.
pub const PAGE_CHUNKS: u32 = PAGE_CELLS / CHUNK_CELLS;

pub const fn zoom_of_level(level: u32) -> u32 {
    PAGE_ZOOM - level
}

// The identity holds by constants agreeing, not by one deriving the rest: the
// X lane is one Mercator world (space/adapter/3857.rs), and a page is 512
// units. Any of them moving breaks the tiling's match with the web map
// convention. How many cells a page is cut into is the scale's, and moves
// nothing here.
const _: () = {
    assert!(L1.count_in(L3) == 512, "a page is one 512-pixel tile");
    assert!(PAGE_ZOOM == 15, "z16 pages are zoom 15 at 512 pixels");
    assert!(TOP_LEVEL <= PAGE_ZOOM, "no level coarser than one page across the world");
};

/// A page of the pyramid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LevelPage {
    pub level: u32,
    /// The page's coordinate at its own level, Z zero.
    pub page: IVec,
}

impl LevelPage {
    pub fn new(level: u32, page: IVec) -> LevelPage {
        LevelPage { level, page: TO_XY(page) }
    }

    /// The level-`level` page a leaf page lies under: its coordinate shifted
    /// right, which on the signed lanes is a floor.
    pub fn over(level: u32, leaf: IVec) -> LevelPage {
        let [x, y] = ancestor_of([ivec::X(leaf), ivec::Y(leaf)], level);
        LevelPage::new(level, ivec::gather(x, y, 0))
    }

    fn at(self) -> [i64; 2] {
        [ivec::X(self.page), ivec::Y(self.page)]
    }

    /// The page this one folds into a level up.
    pub fn parent(self) -> LevelPage {
        let [x, y] = parent_of(self.at());
        LevelPage::new(self.level + 1, ivec::gather(x, y, 0))
    }

    /// The four pages a level down that fold into this one, in `slot_of`'s
    /// order.
    pub fn children(self) -> [LevelPage; 4] {
        std::array::from_fn(|slot| {
            let [x, y] = child_of(self.at(), slot);
            LevelPage::new(self.level - 1, ivec::gather(x, y, 0))
        })
    }

    /// The page as a node of the hierarchy: its side and its coordinate.
    pub fn node(self) -> (Scale, [i64; 2]) {
        (L3.up(self.level as i32), self.at())
    }

    /// The leaf page at this page's south-west corner.
    pub fn first_leaf(self) -> IVec {
        let [x, y] = first_descendant_of(self.at(), self.level);
        ivec::gather(x, y, 0)
    }

    /// The page's side in units.
    pub fn span(self) -> f64 {
        span_of(self.level)
    }

    /// The page's south-west corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let (side, [x, y]) = self.node();
        let base = side.base_of([x, y, 0]);
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), side.side())
    }

    /// The page as a box of the world: its square, and every height a
    /// position reaches over it. A page is stored flat and holds whatever
    /// stands over its square, so no height is outside it.
    pub fn region(self) -> Aabb {
        let (x0, y0, side) = self.rect();
        Aabb {
            min: [x0, y0, REACH.min[2]],
            max: [x0 + side, y0 + side, REACH.max[2]],
        }
    }

    /// A demand's subject: the key with the level in the lane a page does not
    /// use. The URL and the arrival take it apart again.
    pub fn subject(self) -> u64 {
        ivec::gather(ivec::X(self.page), ivec::Y(self.page), self.level as i64)
    }

    pub fn of_subject(subject: u64) -> LevelPage {
        LevelPage::new(ivec::Z(subject) as u32, subject)
    }
}

// The space hierarchy's extents (indexer.rs) are a leaf's: a chunk is 64
// units, a page 512, an anchor 4096. A level is the same hierarchy doubled
// once per level, so every extent a view is measured in scales with the
// level it is measured at.

/// A level-`level` page's side in units.
pub const fn span_of(level: u32) -> f64 {
    L3.up(level as i32).side()
}

/// A level-`level` chunk's side in units.
pub const fn chunk_span_of(level: u32) -> f64 {
    L2.up(level as i32).side()
}

/// A level-`level` anchor's side in units: the floating origin's cell when
/// the window's finest level is `level`.
pub const fn anchor_span_of(level: u32) -> i64 {
    L1.count_in(L4.up(level as i32)) as i64
}

/// The 4×4 pages a level down around a page's four children, in the order a
/// fold reads them: index (qx + 1) + 4·(qy + 1) is the page (qx, qy) from
/// its first child, the page's own children at `CHILDREN`. X wraps, since
/// the lane is one world; a page past Y's edge has no object.
pub fn neighborhood(p: LevelPage) -> [LevelPage; 16] {
    let [x, y] = child_of(p.at(), 0);
    std::array::from_fn(|k| {
        let (qx, qy) = ((k % 4) as i64 - 1, (k / 4) as i64 - 1);
        page_at(p.level - 1, [x + qx, y + qy])
    })
}

/// The level-`level` page at column `x` and row `y`, counted without the
/// seam: X wraps, since the lane is one world, so a column past either end
/// is the one the world meets there.
pub fn page_at(level: u32, [x, y]: [i64; 2]) -> LevelPage {
    let across = 1i64 << (IVEC_X_W - L3.shamt() as u32 - level);
    let wrap = |x: i64| (x + across / 2).rem_euclid(across) - across / 2;
    LevelPage::new(level, ivec::gather(wrap(x), y, 0))
}

/// Where a page's own four children sit in its neighborhood, in `slot_of`'s
/// order: (0, 0), (1, 0), (0, 1), (1, 1) from the first.
pub const CHILDREN: [usize; 4] = [5, 6, 9, 10];

/// A cell of the pyramid: its lattice, the page it lies in and its index
/// there. Made from a point (`Lattice::cell_of`) or a page's own index
/// (`Lattice::cell_at`), so a level and an index that do not belong together
/// cannot be held.
///
/// A block of 2^s × 2^s cells of one level is a cell of the level s above,
/// which is how a block is named here: there is no step beside a level, and
/// so no step that is not a power of two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    lattice: Lattice,
    page: LevelPage,
    index: u32,
}

impl Cell {
    pub fn lattice(self) -> Lattice {
        self.lattice
    }

    pub fn page(self) -> LevelPage {
        self.page
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn level(self) -> u32 {
        self.page.level
    }

    /// The cell's side.
    pub fn scale(self) -> Scale {
        self.lattice.scale(self.page.level)
    }

    /// Its column and row in its page.
    pub fn in_page(self) -> [u32; 2] {
        self.lattice.in_page(self.index)
    }

    /// The point at the cell's south-west corner.
    pub fn base(self) -> Pvf3 {
        let [tx, ty] = self.in_page();
        self.lattice.cell_base(self.page, tx, ty)
    }

    /// The point at the cell's middle on the ground plane.
    pub fn mid(self) -> Pvf3 {
        let base = self.base();
        let half = 1i64 << (FRAC_W as i32 + self.scale().shamt() - 1);
        let mut out = Pvf3::default();
        pvf3::gather(&mut out, pvf3::X(&base) + half, pvf3::Y(&base) + half, 0);
        out
    }

    /// The cell's side in units.
    pub fn side(self) -> f64 {
        self.scale().side()
    }

    /// The cell `levels` above that this one lies in.
    pub fn up(self, levels: u32) -> Cell {
        self.lattice.cell_of(self.page.level + levels, &self.base())
    }

    /// Where `inner`, a cell no coarser than this one and inside it, lies in
    /// it: how many of `inner`'s sides its corner is from this one's, east
    /// and north. Any two lattices', since both are shifts against the unit.
    pub fn offset_of(self, inner: Cell) -> [u32; 2] {
        let (outer, inner_base) = (self.base(), inner.base());
        let shift = FRAC_W as i32 + inner.scale().shamt();
        let lane = |o: i64, i: i64| ((i - o) >> shift) as u32;
        [lane(pvf3::X(&outer), pvf3::X(&inner_base)), lane(pvf3::Y(&outer), pvf3::Y(&inner_base))]
    }

    /// Where this cell lies in the page `levels` above it, in its own sides:
    /// how many of them its corner is from that page's corner, east and
    /// north. A chunk whose page has not come is read from a page over it
    /// (pxl-render mesh/paint/terrain/adapter.rs `ChunkSample::read`), each
    /// of that page's cells standing for a block of its own.
    pub fn in_page_above(self, levels: u32) -> [i64; 2] {
        let up = self.up(levels);
        let [ax, ay] = up.in_page();
        let n = self.scale().count_in(up.scale()) as i64;
        let [ox, oy] = up.offset_of(self);
        [ax as i64 * n + ox as i64, ay as i64 * n + oy as i64]
    }

    /// The `lattice`'s level-`level` cell `offset` of its own sides from this
    /// cell's corner: `offset_of`'s inverse.
    pub fn inner_at(self, lattice: Lattice, level: u32, offset: [u32; 2]) -> Cell {
        let base = self.base();
        let shift = FRAC_W as i32 + lattice.scale(level).shamt();
        let mut at = Pvf3::default();
        pvf3::gather(&mut at, pvf3::X(&base) + ((offset[0] as i64) << shift), pvf3::Y(&base) + ((offset[1] as i64) << shift), 0);
        lattice.cell_of(level, &at)
    }

    /// The center of the cell as a thing standing on `ground` occupies it:
    /// its XY center, and half its side above the ground, since a level's
    /// cells are as tall as they are wide. The one point every layer at this
    /// level stands a thing at.
    pub fn center(self, ground: f64) -> Pvf3 {
        let flat = self.mid();
        let mut at = [0.0; 3];
        projection::unproject(&mut at, flat.i, flat.f);
        let mut out = Pvf3::default();
        projection::project(&mut out, at[0], at[1], ground + self.side() / 2.0);
        out
    }
}

/// A layer's page as the pyramid holds it. Every level of a layer is folded
/// from the level under it by one function, which the bake runs to build the
/// pyramid and the client runs to stand a coarse page up from the fine ones
/// it holds; what the function does with a cell is the layer's own.
///
/// A parent is made from its four children. A layer whose samples reach past
/// a page — terrain's elevation, padded for the bicubic read at its rim —
/// reads the pages around them too, and says so (`sources`); nothing else
/// asks for more than the four.
pub trait Folds: Sized {
    /// The pages of the level below that `p` is folded from.
    fn sources(p: LevelPage) -> Vec<LevelPage> {
        p.children().to_vec()
    }

    /// `p` from its `sources`, as `held` answers for them; None where one it
    /// cannot do without is not held.
    fn fold_page<'a>(p: LevelPage, held: impl Fn(LevelPage) -> Option<&'a Self>) -> Option<Self>
    where
        Self: 'a;

    /// Whether this page, `p`, says all there is of its `child`'s ground, so
    /// the child is no page of its own: the tree stops there, above the finest
    /// level, and the ground under it is read from here. The inverse of the
    /// fold, where the fold loses nothing it would give back. None of a layer
    /// that refines everywhere.
    fn settles(&self, _p: LevelPage, _child: LevelPage) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
