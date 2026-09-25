// pxl-kernel/world/pyramid/chunk.rs

//! The grain a view is held and drawn at: a chunk is 64 × 64 cells of one
//! level, one eighth of that level's page on a side. Pages are what is
//! fetched and kept; a chunk is the finest piece of one that a view asks for
//! at a resolution of its own, since the rings of neighbouring levels meet
//! at chunk edges and not at page edges (window.rs).

use pxl_spec::space::hierarchy::{child_of, parent_of, Scale};
use pxl_spec::space::indexer::L2;
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, CHUNK_CELLS, GROUND, PAGE_CELLS, PAGE_CHUNKS, TOP_LEVEL};

/// A chunk by where it is: its page, and its corner in the page's cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    pub page: LevelPage,
    pub bx: u32,
    pub by: u32,
}

impl Chunk {
    /// The level-`level` chunk a point stands in.
    pub fn of(level: u32, at: &Pvf3) -> Chunk {
        let cell = GROUND.cell_of(level, at);
        let (tx, ty) = (cell.index() % PAGE_CELLS, cell.index() / PAGE_CELLS);
        Chunk {
            page: cell.page(),
            bx: tx & !(CHUNK_CELLS - 1),
            by: ty & !(CHUNK_CELLS - 1),
        }
    }

    /// The chunk of its level `dx` and `dy` chunks along from it.
    pub fn beside(self, dx: i64, dy: i64) -> Chunk {
        let [x, y] = self.at();
        Chunk::from_at(self.page.level, [x + dx, y + dy])
    }

    pub fn level(self) -> u32 {
        self.page.level
    }

    /// The chunk's corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let base = GROUND.cell_base(self.page, self.bx, self.by);
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), chunk_span_of(self.page.level))
    }

    /// The chunk as a node of the hierarchy: its side, the L2 block's at its
    /// level, and its coordinate among its level's chunks.
    pub fn node(self) -> (Scale, [i64; 2]) {
        (L2.up(self.page.level as i32), self.at())
    }

    /// The chunk's coordinate among its level's chunks.
    fn at(self) -> [i64; 2] {
        let across = |page: i64, b: u32| page * PAGE_CHUNKS as i64 + (b / CHUNK_CELLS) as i64;
        [across(ivec::X(self.page.page), self.bx), across(ivec::Y(self.page.page), self.by)]
    }

    fn from_at(level: u32, [x, y]: [i64; 2]) -> Chunk {
        let (per, cells) = (PAGE_CHUNKS as i64, CHUNK_CELLS as i64);
        Chunk {
            page: LevelPage::new(level, ivec::gather(x.div_euclid(per), y.div_euclid(per), 0)),
            bx: (x.rem_euclid(per) * cells) as u32,
            by: (y.rem_euclid(per) * cells) as u32,
        }
    }

    /// The chunk a level up that covers it, none at the pyramid's top.
    pub fn parent(self) -> Option<Chunk> {
        (self.page.level < TOP_LEVEL).then(|| Chunk::from_at(self.page.level + 1, parent_of(self.at())))
    }

    /// Every chunk above it, nearest first.
    pub fn ancestors(self) -> impl Iterator<Item = Chunk> {
        std::iter::successors(self.parent(), |k| k.parent())
    }

    /// The four chunks a level down that cover it, in `slot_of`'s order: a
    /// chunk is 64 cells of its level, which are 128 of the next, in one
    /// page of it.
    pub fn children(self) -> [Chunk; 4] {
        std::array::from_fn(|slot| Chunk::from_at(self.page.level - 1, child_of(self.at(), slot)))
    }
}
