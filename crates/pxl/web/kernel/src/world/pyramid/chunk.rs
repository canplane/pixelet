// pxl-kernel/world/pyramid/chunk.rs

//! The grain a view is held and drawn at: a chunk is 64 × 64 cells of one
//! level, one eighth of that level's page on a side. Pages are what is
//! fetched and kept; a chunk is the finest piece of one that a view asks for
//! at a resolution of its own, since the rings of neighbouring levels meet
//! at chunk edges and not at page edges (window.rs).
//!
//! A chunk is a node of the hierarchy: the L2 block at its level, named by
//! its coordinate among that level's chunks. Its page and its place in the
//! page are derived from that, three levels up, so no pair of them can
//! disagree with where the chunk is.

use pxl_spec::space::hierarchy::{ancestor_of, child_of, first_descendant_of, parent_of, Scale};
use pxl_spec::space::indexer::{L2, L3};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, CHUNK_CELLS, PAGE_CHUNKS, TOP_LEVEL};

/// Levels from a chunk up to the page it lies in.
const PAGE_LEVELS: u32 = L3.above(L2) as u32;

/// A chunk by where it is: its level, and its coordinate among its level's
/// chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    level: u32,
    at: [i64; 2],
}

impl Chunk {
    /// The level-`level` chunk a point stands in.
    pub fn of(level: u32, at: &Pvf3) -> Chunk {
        let [x, y, _] = L2.up(level as i32).index_of(at);
        Chunk { level, at: [x, y] }
    }

    /// The chunks of a page, row by row.
    pub fn of_page(page: LevelPage) -> impl Iterator<Item = Chunk> {
        let [x0, y0] = first_descendant_of(page.node().1, PAGE_LEVELS);
        let n = PAGE_CHUNKS as i64;
        (0..n * n).map(move |k| Chunk {
            level: page.level,
            at: [x0 + k % n, y0 + k / n],
        })
    }

    /// The chunk of its level `dx` and `dy` chunks along from it.
    pub fn beside(self, dx: i64, dy: i64) -> Chunk {
        Chunk {
            level: self.level,
            at: [self.at[0] + dx, self.at[1] + dy],
        }
    }

    pub fn level(self) -> u32 {
        self.level
    }

    /// The page the chunk lies in.
    pub fn page(self) -> LevelPage {
        let [x, y] = ancestor_of(self.at, PAGE_LEVELS);
        LevelPage::new(self.level, ivec::gather(x, y, 0))
    }

    /// The chunk's corner in its page's cells.
    pub fn in_page(self) -> (u32, u32) {
        let [x0, y0] = first_descendant_of(ancestor_of(self.at, PAGE_LEVELS), PAGE_LEVELS);
        ((self.at[0] - x0) as u32 * CHUNK_CELLS, (self.at[1] - y0) as u32 * CHUNK_CELLS)
    }

    /// The point at the chunk's south-west corner.
    pub fn base(self) -> Pvf3 {
        L2.up(self.level as i32).base_of([self.at[0], self.at[1], 0])
    }

    /// The chunk's corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let base = self.base();
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), chunk_span_of(self.level))
    }

    /// The chunk as a node of the hierarchy: its side, the L2 block's at its
    /// level, and its coordinate.
    pub fn node(self) -> (Scale, [i64; 2]) {
        (L2.up(self.level as i32), self.at)
    }

    /// The chunk a level up that covers it, none at the pyramid's top.
    pub fn parent(self) -> Option<Chunk> {
        (self.level < TOP_LEVEL).then(|| Chunk {
            level: self.level + 1,
            at: parent_of(self.at),
        })
    }

    /// Every chunk above it, nearest first.
    pub fn ancestors(self) -> impl Iterator<Item = Chunk> {
        std::iter::successors(self.parent(), |k| k.parent())
    }

    /// The four chunks a level down that cover it, in `slot_of`'s order.
    pub fn children(self) -> [Chunk; 4] {
        std::array::from_fn(|slot| Chunk {
            level: self.level - 1,
            at: child_of(self.at, slot),
        })
    }
}
