// pxl-kernel/world/pyramid/chunk.rs

//! The grain a view is drawn at: a chunk is a cube 64 cells of one level on a
//! side, one eighth of that level's page across. Pages are what is fetched
//! and kept, by what an observation requires (world/pyramid.rs); a chunk is
//! the finest piece of the world a view is drawn in at a resolution of its
//! own, since the levels the rule draws meet at chunk faces and not at page
//! edges (domain.rs). None is fetched for or held.
//!
//! A chunk is a node of the hierarchy in three dimensions: the L2 block at
//! its level, named by its coordinate among that level's blocks in x, y and
//! z, with eight children a level down. Its address is all it is — no table
//! holds one per block, and a block with nothing in it is never made one
//! (domain.rs `Domain::occupied`). What stands in it is stored elsewhere and
//! in two dimensions: the terrain's pages and the objects' columns are read
//! by the column the chunk stands in (`column`), and its page and its place in
//! the page are that column's, three levels up, so no pair of them can
//! disagree with where the chunk is.

use pxl_spec::space::hierarchy::{ancestor_of, child_of, first_descendant_of, parent_of, Scale};
use pxl_spec::space::indexer::{L2, L3};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, CHUNK_CELLS, TOP_LEVEL};

/// The chunks a level down that cover one: two a side in all three.
pub const CHILDREN: usize = 8;

/// Levels from a chunk up to the page it lies in.
const PAGE_LEVELS: u32 = L3.above(L2) as u32;

/// A chunk by where it is: its level, and its coordinate among its level's
/// blocks in x, y and z.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    level: u32,
    at: [i64; 3],
}

impl Chunk {
    /// The level-`level` chunk a point stands in.
    pub fn of(level: u32, at: &Pvf3) -> Chunk {
        Chunk {
            level,
            at: L2.up(level as i32).index_of(at),
        }
    }

    /// Every level-`level` chunk of the column a point stands in, from the
    /// lowest to the highest, as the bounds of a range in chunk order.
    pub fn column_of(level: u32, at: &Pvf3) -> std::ops::RangeInclusive<Chunk> {
        let [x, y, _] = L2.up(level as i32).index_of(at);
        Chunk { level, at: [x, y, i64::MIN] }..=Chunk { level, at: [x, y, i64::MAX] }
    }

    /// The chunk of its level `dx`, `dy` and `dz` chunks along from it.
    pub fn beside(self, dx: i64, dy: i64, dz: i64) -> Chunk {
        let [x, y, z] = self.at;
        Chunk {
            level: self.level,
            at: [x + dx, y + dy, z + dz],
        }
    }

    pub fn level(self) -> u32 {
        self.level
    }

    /// The page the chunk's column lies in.
    pub fn page(self) -> LevelPage {
        let [x, y] = ancestor_of(self.column().1, PAGE_LEVELS);
        LevelPage::new(self.level, ivec::gather(x, y, 0))
    }

    /// The chunk's corner in its page's cells.
    pub fn in_page(self) -> (u32, u32) {
        let [x, y] = self.column().1;
        let [x0, y0] = first_descendant_of(ancestor_of([x, y], PAGE_LEVELS), PAGE_LEVELS);
        ((x - x0) as u32 * CHUNK_CELLS, (y - y0) as u32 * CHUNK_CELLS)
    }

    /// The corner of the chunk's column, in units, at no height: what the
    /// stores the chunk is read from key it by, which are 2D.
    pub fn corner(self) -> IVec {
        let [x, y, _] = self.at;
        L2.up(self.level as i32).base_of([x, y, 0]).i
    }

    /// The chunk's footprint: its corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let base = Pvf3 { i: self.corner(), f: 0 };
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), chunk_span_of(self.level))
    }

    /// The heights the chunk spans, in units: its floor and its ceiling.
    /// From its index and not its corner as a point: a point's height is a
    /// 16-bit lane, ±32,768 units, and a chunk of the pyramid's top is 65,536
    /// tall, so its corner past the first is out of any point's reach.
    pub fn z_range(self) -> (f64, f64) {
        let span = chunk_span_of(self.level);
        let z0 = self.at[2] as f64 * span;
        (z0, z0 + span)
    }

    /// The chunk as a node of the hierarchy: its side, the L2 block's at its
    /// level, and its coordinate.
    pub fn node(self) -> (Scale, [i64; 3]) {
        (L2.up(self.level as i32), self.at)
    }

    /// The column the chunk stands in, as a node in two dimensions: what the
    /// terrain's pages and the objects' store are read by.
    pub fn column(self) -> (Scale, [i64; 2]) {
        let [x, y, _] = self.at;
        (L2.up(self.level as i32), [x, y])
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

    /// The eight chunks a level down that cover it, in `slot_of`'s order.
    pub fn children(self) -> [Chunk; CHILDREN] {
        std::array::from_fn(|slot| Chunk {
            level: self.level - 1,
            at: child_of(self.at, slot),
        })
    }
}
