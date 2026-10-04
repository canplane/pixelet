// pxl-kernel/world/pyramid/chunk.rs

//! A chunk: a cube of 64 cells of one zoom, the grain a view is drawn at.
//! Never fetched or held; only its address exists, and an empty one is never
//! made. Its page and place in the page derive from its column, so they
//! cannot disagree with where it is.

use pxl_spec::space::hierarchy::{ancestor_of, child_of, offset_in, parent_of, Scale};
use pxl_spec::space::indexer::{L2, L3};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::{chunk_scale, chunk_span_of, page_at, Page, CHUNK_CELLS, PAGE_CELLS, TOP_Z};
use pxl_spec::space::v3::Aabb;

pub const CHILDREN: usize = 8;

/// A page is this many steps over a chunk.
const PAGE_STEPS: u32 = L3.above(L2) as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    /// The zoom of its cells.
    z: u32,
    at: [i64; 3],
}

impl Chunk {
    pub fn of(z: u32, at: &Pvf3) -> Chunk {
        Chunk { z, at: chunk_scale(z).index_of(at) }
    }

    pub fn at(z: u32, at: [i64; 3]) -> Chunk {
        Chunk { z, at }
    }

    /// As the bounds of a range in chunk order.
    pub fn column_of(z: u32, at: &Pvf3) -> std::ops::RangeInclusive<Chunk> {
        let [x, y, _] = chunk_scale(z).index_of(at);
        Chunk { z, at: [x, y, i64::MIN] }..=Chunk { z, at: [x, y, i64::MAX] }
    }

    pub fn beside(self, dx: i64, dy: i64, dh: i64) -> Chunk {
        let [x, y, h] = self.at;
        Chunk {
            z: self.z,
            at: [x + dx, y + dy, h + dh],
        }
    }

    pub fn z(self) -> u32 {
        self.z
    }

    pub fn page(self) -> Page {
        let [x, y] = ancestor_of(self.column().1, PAGE_STEPS);
        Page::new(self.z, ivec::gather(x, y, 0))
    }

    pub fn in_page(self) -> (u32, u32) {
        let [x, y] = offset_in(self.column().1, PAGE_STEPS);
        (x as u32 * CHUNK_CELLS, y as u32 * CHUNK_CELLS)
    }

    /// Neighbouring pages an apron of `apron` cells reaches into, with their
    /// offsets from the chunk's page; none for a chunk inside it.
    pub fn pages_beside(self, apron: u32) -> impl Iterator<Item = ([i64; 2], Page)> {
        let (x0, y0) = self.in_page();
        let steps = move |v: u32| {
            [0, -1, 1]
                .into_iter()
                .filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + CHUNK_CELLS + apron > PAGE_CELLS))
        };
        let page = self.page();
        let [px, py] = [ivec::X(page.page), ivec::Y(page.page)];
        let z = self.z;
        steps(y0)
            .flat_map(move |qy| steps(x0).map(move |qx| [qx, qy]))
            .filter(|&q| q != [0, 0])
            .map(move |[qx, qy]| ([qx, qy], page_at(z, [px + qx, py + qy])))
    }

    /// The column's corner at no height: what the 2D stores key it by.
    pub fn corner(self) -> IVec {
        let [x, y, _] = self.at;
        chunk_scale(self.z).base_of([x, y, 0]).i
    }

    pub fn rect(self) -> (f64, f64, f64) {
        let base = Pvf3 { i: self.corner(), f: 0 };
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), chunk_span_of(self.z))
    }

    /// From the index, not the corner point: a point's height is 16 bits
    /// (±32,768 units) and a z6 chunk is 65,536 tall.
    pub fn z_range(self) -> (f64, f64) {
        let span = chunk_span_of(self.z);
        let z0 = self.at[2] as f64 * span;
        (z0, z0 + span)
    }

    pub fn region(self) -> Aabb {
        let span = chunk_span_of(self.z);
        let (x0, y0, _) = self.rect();
        let z0 = self.z_range().0;
        Aabb {
            min: [x0, y0, z0],
            max: [x0 + span, y0 + span, z0 + span],
        }
    }

    pub fn node(self) -> (Scale, [i64; 3]) {
        (chunk_scale(self.z), self.at)
    }

    pub fn column(self) -> (Scale, [i64; 2]) {
        let [x, y, _] = self.at;
        (chunk_scale(self.z), [x, y])
    }

    pub fn parent(self) -> Option<Chunk> {
        (self.z > TOP_Z).then(|| Chunk {
            z: self.z - 1,
            at: parent_of(self.at),
        })
    }

    pub fn ancestors(self) -> impl Iterator<Item = Chunk> {
        std::iter::successors(self.parent(), |k| k.parent())
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [Chunk; CHILDREN] {
        std::array::from_fn(|slot| Chunk {
            z: self.z + 1,
            at: child_of(self.at, slot),
        })
    }
}
