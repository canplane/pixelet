// pxl-spec/space/pyramid.rs

//! The pyramid: the space hierarchy (indexer.rs) repeated over coarser
//! blocks. A level-k cell is a block of 2^k units and a level-k page is a
//! page's worth of those cells, 2^k leaf pages on a side, so every extent the
//! hierarchy gives a leaf is the same extent at level k, scaled
//! (issue/20260921-level-step-and-zoom-are-one-number.md).
//!
//! It is the one hierarchy every layer is held in. Where a cell is, which
//! page it belongs to, which pages fold into which, and where a cell's centre
//! stands are decided here and nowhere else; a layer owns only what a cell
//! carries and how a parent's cell is made from its children's (`Folds`).

use super::indexer::{L2_SHAMT, L3_IDX, L3_SHAMT, L4_RANGE, SPAN_CENTER};
use super::pvf3::core::IVEC_X_W;
use super::pvf3::ivec::{self, IVec};
use super::pvf3::projection;
use super::pvf3::pvf3::Pvf3;
use super::xy::TO_XY;

/// The coarsest level the pyramid is folded to. At level 10 — zoom 5 — a
/// page spans about 9° of latitude, across which the page-constant vertical
/// scale is already 12% off: past it Web Mercator on a plane stops being
/// honest, and a coarser view is a different projection over the same tiles
/// rather than another level. A bake folds up to it and a server answers
/// nothing above it.
pub const TOP_LEVEL: u32 = 10;

/// The zoom a level-0 page is, in the 512-pixel tile convention: the X lane
/// is one EPSG:3857 world, so a level has 2^(IVEC_X_W − L3_SHAMT − level)
/// pages across it, which is a zoom's tile count. Level k is zoom
/// LEVEL0_ZOOM − k, and a level-0 cell is the standard 256-pixel z16 pixel
/// exactly.
pub const LEVEL0_ZOOM: u32 = IVEC_X_W - L3_SHAMT;

/// Cells on a page's side, at every level. A level-0 cell is the unit, so
/// at level 0 this is the L3 page's side in units.
pub const PAGE_CELLS: u32 = 1 << L3_SHAMT;

/// Cells on a chunk's side, at every level: the L2 block's.
pub const CHUNK_CELLS: u32 = 1 << L2_SHAMT;

/// Chunks on a page's side.
pub const PAGE_CHUNKS: u32 = PAGE_CELLS / CHUNK_CELLS;

pub const fn zoom_of_level(level: u32) -> u32 {
    LEVEL0_ZOOM - level
}

// The identity holds by constants agreeing, not by one deriving the rest: the
// X lane is one Mercator world (space/adapter/3857.rs), a level-0 cell is one
// unit, and a page is 512 cells. Any of them moving breaks the tiling's match
// with the web map convention.
const _: () = {
    assert!(PAGE_CELLS == 512, "a page is one 512-pixel tile");
    assert!(LEVEL0_ZOOM == 15, "level 0 is zoom 15 at 512 pixels, z16 at 256");
    assert!(TOP_LEVEL <= LEVEL0_ZOOM, "no level coarser than one page across the world");
};

// ---- the hierarchy, in any number of dimensions ----
//
// What a level is to the one under it does not depend on how many axes
// there are: a node's parent is its coordinate halved, and it is one of its
// parent's 2^D children, told apart by the low bit of each axis. The ground's
// pages and cells are this in two (`LevelPage`, `Cell`), an object's cells
// in three (pxl-render mesh/paint/object/fold.rs), and a fold reads the
// children in this order whichever it is (bake/fold.rs `majority`).

/// The node a level up that `at` folds into: every axis halved, a floor on
/// the signed lanes.
pub fn parent_of<const D: usize>(at: [i64; D]) -> [i64; D] {
    at.map(|v| v.div_euclid(2))
}

/// Which of its parent's 2^D children `at` is: axis i's low bit at bit i,
/// `x + 2y` in two dimensions and `x + 2y + 4z` in three.
pub fn slot_of<const D: usize>(at: [i64; D]) -> usize {
    (0..D).map(|i| (at[i].rem_euclid(2) as usize) << i).sum()
}

/// The child in `slot` of the node at `at`: `parent_of` and `slot_of`'s
/// inverse.
pub fn child_of<const D: usize>(at: [i64; D], slot: usize) -> [i64; D] {
    std::array::from_fn(|i| 2 * at[i] + ((slot >> i) & 1) as i64)
}

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
        let s = 1i64 << level;
        LevelPage::new(level, ivec::gather(ivec::X(leaf).div_euclid(s), ivec::Y(leaf).div_euclid(s), 0))
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

    /// The leaf page at this page's south-west corner.
    pub fn first_leaf(self) -> IVec {
        ivec::gather(ivec::X(self.page) << self.level, ivec::Y(self.page) << self.level, 0)
    }

    /// The page's side in units.
    pub fn span(self) -> f64 {
        span_of(self.level)
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
// leaf blocks, a page 512, an anchor 4096. A level is the same hierarchy over
// its own blocks, 2^level units each, so every extent a view is measured in
// scales with the level it is measured at.

/// A level-`level` page's side in units.
pub const fn span_of(level: u32) -> f64 {
    ((PAGE_CELLS as u64) << level) as f64
}

/// A level-`level` chunk's side in units.
pub const fn chunk_span_of(level: u32) -> f64 {
    ((CHUNK_CELLS as u64) << level) as f64
}

/// A level-`level` anchor's side in units: the floating origin's cell when
/// the window's finest level is `level`.
pub const fn anchor_span_of(level: u32) -> i64 {
    (L4_RANGE as i64) << level
}

/// The 4×4 pages a level down around a page's four children, in the order a
/// fold reads them: index (qx + 1) + 4·(qy + 1) is the page at
/// (2X + qx, 2Y + qy), the page's own children at `CHILDREN`. X wraps, since
/// the lane is one world; a page past Y's edge has no object.
pub fn neighbourhood(p: LevelPage) -> [LevelPage; 16] {
    let below = p.level - 1;
    let across = 1i64 << (IVEC_X_W - L3_SHAMT - below);
    let wrap = |x: i64| (x + across / 2).rem_euclid(across) - across / 2;
    let (x, y) = (ivec::X(p.page), ivec::Y(p.page));
    std::array::from_fn(|k| {
        let (qx, qy) = ((k % 4) as i64 - 1, (k / 4) as i64 - 1);
        LevelPage::new(below, ivec::gather(wrap(2 * x + qx), 2 * y + qy, 0))
    })
}

/// Where a page's own four children sit in its neighbourhood, in quadrant
/// order: (0, 0), (1, 0), (0, 1), (1, 1).
pub const CHILDREN: [usize; 4] = [5, 6, 9, 10];

/// A cell's index in its page: `ty · PAGE_CELLS + tx`.
pub fn cell_index(tx: u32, ty: u32) -> u32 {
    ty * PAGE_CELLS + tx
}

/// A cell of the pyramid: the page it lies in and its index there. Made from
/// an address (`Cell::of`) or a page's own index (`Cell::at`), so a level
/// and an index that do not belong together cannot be held.
///
/// A block of 2^s × 2^s cells of one level is a cell of the level s above,
/// which is how a block is named here: there is no step beside a level, and
/// so no step that is not a power of two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    page: LevelPage,
    index: u32,
}

impl Cell {
    /// The level-`level` cell a unit address stands in.
    pub fn of(level: u32, addr: IVec) -> Cell {
        let page = LevelPage::over(level, L3_IDX(addr));
        let at = |lane: i64, page_lane: i64| (lane.div_euclid(1 << level) - (page_lane << L3_SHAMT)) as u32;
        Cell {
            page,
            index: cell_index(at(ivec::X(addr), ivec::X(page.page)), at(ivec::Y(addr), ivec::Y(page.page))),
        }
    }

    /// The cell at `index` of `page`.
    pub fn at(page: LevelPage, index: u32) -> Cell {
        assert!(index < PAGE_CELLS * PAGE_CELLS, "a cell index is inside its page");
        Cell { page, index }
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

    /// The unit address of the cell's south-west corner.
    pub fn base(self) -> IVec {
        cell_base(self.page, self.index % PAGE_CELLS, self.index / PAGE_CELLS)
    }

    /// The unit address at the cell's middle: its corner and half its side
    /// on the ground plane, the corner itself for a level-0 cell, whose side
    /// is one unit.
    pub fn mid(self) -> IVec {
        let base = self.base();
        let half = (1i64 << self.page.level) / 2;
        ivec::gather(ivec::X(base) + half, ivec::Y(base) + half, 0)
    }

    /// The cell's side in units.
    pub fn side(self) -> f64 {
        f64::from(1u32 << self.page.level)
    }

    /// The cell `levels` above that this one lies in.
    pub fn up(self, levels: u32) -> Cell {
        Cell::of(self.page.level + levels, self.base())
    }

    /// The centre of the cell as a thing standing on `ground` occupies it:
    /// its XY centre, and half its side above the ground, since a level's
    /// cells are as tall as they are wide. The one point every layer at this
    /// level stands a thing at.
    pub fn centre(self, ground: f64) -> Pvf3 {
        let mut flat = Pvf3::default();
        SPAN_CENTER(&mut flat, self.base(), self.page.level);
        let mut at = [0.0; 3];
        projection::unproject(&mut at, flat.i, flat.f);
        let mut out = Pvf3::default();
        projection::project(&mut out, at[0], at[1], ground + self.side() / 2.0);
        out
    }
}

/// The unit address of the south-west corner of the cell at (tx, ty) in
/// `page`.
pub fn cell_base(page: LevelPage, tx: u32, ty: u32) -> IVec {
    let first = |p: i64, t: u32| ((p << L3_SHAMT) + i64::from(t)) << page.level;
    ivec::gather(first(ivec::X(page.page), tx), first(ivec::Y(page.page), ty), 0)
}

/// The parent cell a child's cell folds into, the child at quadrant `q`.
pub fn parent_cell(q: usize, index: u32) -> u32 {
    let half = PAGE_CELLS / 2;
    let (tx, ty) = (index % PAGE_CELLS, index / PAGE_CELLS);
    cell_index((q as u32 % 2) * half + tx / 2, (q as u32 / 2) * half + ty / 2)
}

/// The child quadrant and the first of the 2×2 child cells a parent's cell
/// is folded from: `parent_cell`'s inverse.
pub fn child_block(index: u32) -> (usize, u32) {
    let half = PAGE_CELLS / 2;
    let (tx, ty) = (index % PAGE_CELLS, index / PAGE_CELLS);
    ((tx / half + 2 * (ty / half)) as usize, cell_index((2 * tx) % PAGE_CELLS, (2 * ty) % PAGE_CELLS))
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
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
