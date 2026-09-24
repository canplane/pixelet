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

use super::indexer::{L3_IDX, L3_SHAMT, SPAN_CENTER};
use super::pvf3::core::IVEC_X_W;
use super::pvf3::ivec::{self, IVec};
use super::pvf3::pvf3::{self, Pvf3};
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

/// Cells on a page's side, at every level.
pub const PAGE_CELLS: u32 = 1 << L3_SHAMT;

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

    /// The page this one folds into a level up.
    pub fn parent(self) -> LevelPage {
        LevelPage {
            level: self.level + 1,
            ..LevelPage::over(1, self.page)
        }
    }

    /// The four pages a level down that fold into this one, in `CHILDREN`'s
    /// quadrant order.
    pub fn children(self) -> [LevelPage; 4] {
        let (x, y) = (ivec::X(self.page) * 2, ivec::Y(self.page) * 2);
        [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| LevelPage::new(self.level - 1, ivec::gather(x + dx, y + dy, 0)))
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

/// A level-`level` page's side in units.
pub fn span_of(level: u32) -> f64 {
    (u64::from(PAGE_CELLS) << level) as f64
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

/// The level-`level` cell a unit address stands in: its page, and its index
/// in that page.
pub fn cell_of(level: u32, addr: IVec) -> (LevelPage, u32) {
    let page = LevelPage::over(level, L3_IDX(addr));
    let at = |lane: i64, page_lane: i64| (lane.div_euclid(1 << level) - (page_lane << L3_SHAMT)) as u32;
    (page, cell_index(at(ivec::X(addr), ivec::X(page.page)), at(ivec::Y(addr), ivec::Y(page.page))))
}

/// The centre of the block of `step` × `step` cells a cell stands in, on
/// the ground plane: the one XY every layer at this level stands a thing at.
/// Z is the layer's to find.
pub fn cell_centre(page: LevelPage, index: u32, step: u32) -> Pvf3 {
    let (tx, ty) = ((index % PAGE_CELLS) as i64, (index / PAGE_CELLS) as i64);
    let first = |p: i64, t: i64| ((p << L3_SHAMT) + t) << page.level;
    let addr = ivec::gather(first(ivec::X(page.page), tx), first(ivec::Y(page.page), ty), 0);
    let mut out = Pvf3::default();
    SPAN_CENTER(&mut out, addr, page.level + step.trailing_zeros());
    let (x, y) = (pvf3::X(&out), pvf3::Y(&out));
    pvf3::gather(&mut out, x, y, 0);
    out
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
/// `around` is `neighbourhood`'s block; a layer that needs no ring reads
/// `CHILDREN` alone. None where a page the layer needs is not held.
pub trait Folds: Sized {
    fn fold_held(around: [Option<&Self>; 16], level: u32) -> Option<Self>;
}
