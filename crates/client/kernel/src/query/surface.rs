// pxl-kernel/query/surface.rs

//! The ground's face over a node at once, a ring of its tile's cells
//! around it too: each cell's top and look as the chunks drawing it compose
//! them, its bridges' and tunnels' at grade included, read in one pass
//! rather than cell by cell. What a policy over the ground's face — where
//! decor stands, which way it faces — reads, knowing no tile.

use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::block::sample::{beside_index, Around, ChunkSample, APRON};
use pxl_spec::space::coord::pvf3::ivec;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::global::Global;

use super::block::Surface;
use super::world::World;
use crate::layer::{Answer, Query};

/// A node's cells and the ring around them, in its tile's cells.
pub struct Surfaces<'s> {
    sample: &'s ChunkSample,
    /// The first cell read, the ring's.
    first: [i64; 2],
}

impl Surfaces<'_> {
    /// The surface of the tile's cell `at`; none where it has no ground or
    /// was not read.
    pub fn at(&self, at: [i64; 2]) -> Option<Surface> {
        let span = i64::from(self.sample.span);
        let [x, y] = [at[0] - self.first[0], at[1] - self.first[1]];
        if !(0..span).contains(&x) || !(0..span).contains(&y) {
            return None;
        }
        let i = (y * span + x) as usize;
        self.sample.has_ground(i).then(|| Surface {
            top: f64::from(self.sample.ele[i]),
            block: self.sample.block[i],
        })
    }

    /// The look of the tile's cell `at`, as `at` gives it.
    pub fn look(&self, at: [i64; 2]) -> Option<GroundId> {
        self.at(at).map(|s| s.block)
    }
}

static SAMPLE: Global<ChunkSample> = Global::new();

impl World<'_> {
    /// The ground's face over the node `node` of grid cells at `cell`, read
    /// off the tile those cells read as a chunk is, and `ring` of its cells
    /// past it on every side, the tiles beside included; none inside where
    /// the heap refused it room. `f` asks the world nothing: it reads the
    /// query's one scratch.
    pub fn surfaces<R>(&self, node: Node<2>, cell: Zoom, ring: u32, f: impl FnOnce(&Surfaces) -> R) -> Answer<Option<R>> {
        let mut beside = [None; 8];
        for ([qx, qy], p) in GROUND.read_beside(node, cell, ring) {
            // A tile is read at its own cells.
            beside[beside_index(qx, qy)] = self.terrain.read((p.zoom, [ivec::X(p.tile), ivec::Y(p.tile), 0]), GROUND.grid_zoom(p.zoom)).value();
        }
        let answer = self.terrain.read((node.0, [node.1[0], node.1[1], 0]), cell);
        let own = match answer {
            Answer::Exact(own) | Answer::Coarser(own, _) => own,
            Answer::Nothing(lack) => return Answer::Nothing(lack),
        };
        let around = Around {
            own: own.view(),
            beside: beside.map(|b| b.map(|s| s.view())),
        };
        let (x0, y0) = GROUND.read_at(node, cell);
        let span = GROUND.read_side(node.0, cell) + 2 * ring;
        let first = [i64::from(x0) - i64::from(ring), i64::from(y0) - i64::from(ring)];
        let sample = SAMPLE.get(|| ChunkSample::of_span(span));
        if sample.block.len() < (span * span) as usize {
            *sample = ChunkSample::of_span(span);
        }
        // `read` reads from an apron's width before its first.
        let read = sample.read(&around, first.map(|v| v + i64::from(APRON)), span, GROUND.cell_zoom(GROUND.read_zoom(cell))).is_ok();
        let sample = &*sample;
        answer.map(|_| read.then(|| f(&Surfaces { sample, first })))
    }
}
