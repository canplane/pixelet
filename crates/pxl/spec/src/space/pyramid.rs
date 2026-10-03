// pxl-spec/space/pyramid.rs

//! The pyramid: the space hierarchy repeated over coarser blocks. A level-k
//! cell is 2^k of its lattice's first cells on a side and a level-k page is
//! 2^k leaf pages on a side. Cell placement, page membership, folding order
//! and cell centers are decided here only; a layer owns what a cell carries
//! and how it folds (`Folds`).

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

/// The coarsest folded level. At z6 a page spans about 9° of latitude and the
/// page-constant vertical scale is already 12% off.
pub const TOP_LEVEL: u32 = 10;

// A lattice is a scale against the unit: its level-k cell is k levels above
// its first. Pages and chunks are fixed in units (L3, L2).

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

    pub const fn scale(self, level: u32) -> Scale {
        self.cell.up(level as i32)
    }

    /// `page_cells` as a shift.
    pub const fn page_cell_shamt(self) -> u32 {
        L3.above(self.cell) as u32
    }

    /// How many of its first level's cells stand across one at `level`.
    pub const fn finest_in(self, level: u32) -> u32 {
        self.scale(0).count_in(self.scale(level))
    }

    pub const fn page_cells(self) -> u32 {
        self.cell.count_in(L3)
    }

    pub const fn chunk_cells(self) -> u32 {
        self.cell.count_in(L2)
    }

    pub const fn cell_index(self, tx: u32, ty: u32) -> u32 {
        ty * self.page_cells() + tx
    }

    /// The level-`level` cell a point stands in; a point, since a cell may be finer than a unit.
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

    pub fn cell_at(self, page: LevelPage, index: u32) -> Cell {
        xpute_core::ensure!(index < self.page_cells() * self.page_cells(), ENOTRECOVERABLE);
        Cell { lattice: self, page, index }
    }

    pub fn cell_base(self, page: LevelPage, tx: u32, ty: u32) -> Pvf3 {
        let [x0, y0] = first_descendant_of([ivec::X(page.page), ivec::Y(page.page)], self.page_cell_shamt());
        self.scale(page.level).base_of([x0 + i64::from(tx), y0 + i64::from(ty), 0])
    }

    // A page's four children form one 2n × 2n block a level down, slot q at (q & 1, q >> 1).

    pub fn parent_cell(self, q: usize, index: u32) -> u32 {
        let n = self.page_cells() as i64;
        let (page, [tx, ty]) = (child_of([0, 0], q), self.in_page(index).map(i64::from));
        let [x, y] = parent_of([page[0] * n + tx, page[1] * n + ty]);
        self.cell_index(x as u32, y as u32)
    }

    /// The child slot and the 2×2 cells, in `slot_of`'s order, a parent cell
    /// folds from: `parent_cell`'s inverse.
    pub fn child_block(self, index: u32) -> (usize, [u32; 4]) {
        let n = self.page_cells() as i64;
        let [x, y] = child_of(self.in_page(index).map(i64::from), 0);
        let [x0, y0] = [(x % n) as u32, (y % n) as u32];
        let cells = std::array::from_fn(|q| self.cell_index(x0 + (q & 1) as u32, y0 + (q >> 1) as u32));
        (slot_of([x / n, y / n]), cells)
    }

    /// The cells of this lattice under `outer`'s cell `index` of the same page, row by row.
    pub fn under(self, outer: Lattice, index: u32) -> impl Iterator<Item = u32> {
        let n = self.cell.count_in(outer.cell);
        let [x0, y0] = outer.in_page(index).map(|v| v * n);
        (0..n).flat_map(move |dy| (0..n).map(move |dx| self.cell_index(x0 + dx, y0 + dy)))
    }

    pub const fn in_page(self, index: u32) -> [u32; 2] {
        let n = self.page_cells();
        [index % n, index / n]
    }

    // A page's aligned 2^j blocks for j from 1 up, smallest size first, each size row by row.

    pub const fn page_blocks(self) -> u32 {
        self.first_block(self.page_cell_shamt() + 1)
    }

    pub const fn block_index(self, j: u32, tx: u32, ty: u32) -> u32 {
        if j == 0 || j > self.page_cell_shamt() {
            panic!()
        }
        self.first_block(j) + (ty >> j) * (self.page_cells() >> j) + (tx >> j)
    }

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

pub const GROUND: Lattice = Lattice::new(CELL);

/// The index lane's lattice (bake/format.rs `Index`).
pub const EXTENT: Lattice = Lattice::new(EXTENT_CELL);

pub const DECK: Lattice = Lattice::new(DECK_CELL);

pub const PAGE_CELLS: u32 = GROUND.page_cells();

pub const CHUNK_CELLS: u32 = GROUND.chunk_cells();

pub const PAGE_CHUNKS: u32 = PAGE_CELLS / CHUNK_CELLS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LevelPage {
    pub level: u32,
    /// Z is zero.
    pub page: IVec,
}

impl LevelPage {
    pub fn new(level: u32, page: IVec) -> LevelPage {
        LevelPage { level, page: TO_XY(page) }
    }

    /// The level-`level` page a leaf page lies under.
    pub fn over(level: u32, leaf: IVec) -> LevelPage {
        let [x, y] = ancestor_of([ivec::X(leaf), ivec::Y(leaf)], level);
        LevelPage::new(level, ivec::gather(x, y, 0))
    }

    fn at(self) -> [i64; 2] {
        [ivec::X(self.page), ivec::Y(self.page)]
    }

    pub fn parent(self) -> LevelPage {
        let [x, y] = parent_of(self.at());
        LevelPage::new(self.level + 1, ivec::gather(x, y, 0))
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [LevelPage; 4] {
        std::array::from_fn(|slot| {
            let [x, y] = child_of(self.at(), slot);
            LevelPage::new(self.level - 1, ivec::gather(x, y, 0))
        })
    }

    pub fn node(self) -> (Scale, [i64; 2]) {
        (L3.up(self.level as i32), self.at())
    }

    pub fn first_leaf(self) -> IVec {
        let [x, y] = first_descendant_of(self.at(), self.level);
        ivec::gather(x, y, 0)
    }

    pub fn span(self) -> f64 {
        span_of(self.level)
    }

    /// South-west corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let (side, [x, y]) = self.node();
        let base = side.base_of([x, y, 0]);
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), side.side())
    }

    /// The page as a box of the world: its square, every height.
    pub fn region(self) -> Aabb {
        let (x0, y0, side) = self.rect();
        Aabb {
            min: [x0, y0, REACH.min[2]],
            max: [x0 + side, y0 + side, REACH.max[2]],
        }
    }

    /// A demand's subject: the key with the level in the Z lane.
    pub fn subject(self) -> u64 {
        ivec::gather(ivec::X(self.page), ivec::Y(self.page), self.level as i64)
    }

    pub fn of_subject(subject: u64) -> LevelPage {
        LevelPage::new(ivec::Z(subject) as u32, subject)
    }
}

pub const fn span_of(level: u32) -> f64 {
    L3.up(level as i32).side()
}

/// A level-`level` chunk's side.
pub const fn chunk_scale(level: u32) -> Scale {
    L2.up(level as i32)
}

pub const fn chunk_span_of(level: u32) -> f64 {
    chunk_scale(level).side()
}

/// A level-`level` anchor's side in units: the floating origin's cell.
pub const fn anchor_span_of(level: u32) -> i64 {
    L1.count_in(L4.up(level as i32)) as i64
}

/// The 4×4 pages a level down around a page's children, each at
/// `neighbor_index`; X wraps and a page past Y's edge has no object.
pub fn neighborhood(p: LevelPage) -> [LevelPage; 16] {
    let [x, y] = child_of(p.at(), 0);
    std::array::from_fn(|k| {
        let (qx, qy) = ((k % 4) as i64 - 1, (k / 4) as i64 - 1);
        page_at(p.level - 1, [x + qx, y + qy])
    })
}

/// Where the page (qx, qy) from a parent's first child, each −1 to 2, lies in
/// its `neighborhood`.
pub const fn neighbor_index(qx: i64, qy: i64) -> usize {
    ((qx + 1) + 4 * (qy + 1)) as usize
}

/// The level-`level` page at (x, y), X wrapped around the world.
pub fn page_at(level: u32, [x, y]: [i64; 2]) -> LevelPage {
    let across = 1i64 << (IVEC_X_W - L3.shamt() as u32 - level);
    let wrap = |x: i64| (x + across / 2).rem_euclid(across) - across / 2;
    LevelPage::new(level, ivec::gather(wrap(x), y, 0))
}

/// Where a page's own children sit in its neighborhood, in `slot_of`'s order.
pub const CHILDREN: [usize; 4] = [5, 6, 9, 10];

/// A cell of the pyramid: lattice, page and index. Built only from a point
/// or a page's own index, so a mismatched level and index cannot be held.
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

    pub fn scale(self) -> Scale {
        self.lattice.scale(self.page.level)
    }

    pub fn in_page(self) -> [u32; 2] {
        self.lattice.in_page(self.index)
    }

    pub fn base(self) -> Pvf3 {
        let [tx, ty] = self.in_page();
        self.lattice.cell_base(self.page, tx, ty)
    }

    pub fn mid(self) -> Pvf3 {
        let base = self.base();
        let half = 1i64 << (FRAC_W as i32 + self.scale().shamt() - 1);
        pvf3::gather(pvf3::X(&base) + half, pvf3::Y(&base) + half, 0)
    }

    pub fn side(self) -> f64 {
        self.scale().side()
    }

    pub fn up(self, levels: u32) -> Cell {
        self.lattice.cell_of(self.page.level + levels, &self.base())
    }

    /// Where `inner`, no coarser and inside this cell, lies in it, in `inner`'s sides.
    pub fn offset_of(self, inner: Cell) -> [u32; 2] {
        let (outer, inner_base) = (self.base(), inner.base());
        let shift = FRAC_W as i32 + inner.scale().shamt();
        let lane = |o: i64, i: i64| ((i - o) >> shift) as u32;
        [lane(pvf3::X(&outer), pvf3::X(&inner_base)), lane(pvf3::Y(&outer), pvf3::Y(&inner_base))]
    }

    /// Where this cell lies in the page `levels` above it, in its own sides.
    pub fn in_page_above(self, levels: u32) -> [i64; 2] {
        let up = self.up(levels);
        let [ax, ay] = up.in_page();
        let n = self.scale().count_in(up.scale()) as i64;
        let [ox, oy] = up.offset_of(self);
        [ax as i64 * n + ox as i64, ay as i64 * n + oy as i64]
    }

    /// `offset_of`'s inverse.
    pub fn inner_at(self, lattice: Lattice, level: u32, offset: [u32; 2]) -> Cell {
        let base = self.base();
        let shift = FRAC_W as i32 + lattice.scale(level).shamt();
        let at = pvf3::gather(pvf3::X(&base) + ((offset[0] as i64) << shift), pvf3::Y(&base) + ((offset[1] as i64) << shift), 0);
        lattice.cell_of(level, &at)
    }

    /// The point every layer stands a thing at: the XY center, half a side above
    /// `ground`, since a level's cells are as tall as they are wide.
    pub fn center(self, ground: f64) -> Pvf3 {
        let flat = self.mid();
        let at = projection::unproject(flat.i, flat.f);
        projection::project(at[0], at[1], ground + self.side() / 2.0)
    }
}

/// A layer's page as the pyramid holds it. One fold makes each level from the
/// one below, run by the bake and by the client. A parent reads its four
/// children unless `sources` says otherwise.
pub trait Folds: Sized {
    fn sources(p: LevelPage) -> Vec<LevelPage> {
        p.children().to_vec()
    }

    /// `p` from its sources; None where a required one is not held.
    fn fold_page<'a>(p: LevelPage, held: impl Fn(LevelPage) -> Option<&'a Self>) -> Option<Self>
    where
        Self: 'a;

    /// Whether `p` already says all of `child`'s ground, so the tree stops
    /// there and the child is never a page of its own.
    fn settles(&self, _p: LevelPage, _child: LevelPage) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
