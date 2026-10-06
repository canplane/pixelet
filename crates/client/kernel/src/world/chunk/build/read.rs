// pxl-kernel/world/chunk/build/read.rs

//! What a chunk reads of the terrain, through the query: its tile and the
//! tiles its apron reaches beside it.

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::coord::pvf3::ivec;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::offset_in;
use pxl_spec::space::layer::lattice::GROUND;

use pxl_render::mesh::paint::terrain::adapter::{beside_index, Around};
use pxl_render::mesh::run::APRON;

use crate::layer::terrain::Source;
use crate::layer::{Answer, Query};
use crate::world::chunk::Chunk as PyramidChunk;
use crate::world::pyramid::Layer as _;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use super::*;

/// A chunk's tile and each apron neighbour, from what that neighbour is
/// drawn from, so a shared edge is one surface.
#[derive(Clone, Copy)]
pub(super) struct Read {
    pub(super) own: Source<'static>,
    pub(super) beside: [Option<Source<'static>>; 8],
    pub(super) first: [i64; 2],
}

impl Read {
    pub(super) fn around(&self) -> Around<'static> {
        Around {
            own: self.own.view(),
            beside: self.beside.map(|b| b.as_ref().map(Source::view)),
        }
    }
}

/// The ground a chunk is built from, as the terrain answers it; what a build
/// reads and what it is keyed by are this one reading.
pub(crate) fn ground_of(leaf: PyramidChunk) -> (Answer<Source<'static>>, [Option<Source<'static>>; 8]) {
    let terrain = Terrain::pyramid().residency();
    let mut beside = [None; 8];
    for ([qx, qy], p) in GROUND.read_beside(leaf.column(), leaf.cell_zoom(), APRON) {
        // A tile is read at its own cells.
        beside[beside_index(qx, qy)] = terrain.read((p.zoom, [ivec::X(p.tile), ivec::Y(p.tile), 0]), GROUND.grid_zoom(p.zoom)).value();
    }
    (terrain.read(leaf.node(), leaf.cell_zoom()), beside)
}

/// Zooms runs at `runs` are finer than `c`'s tile's cells: none but past the leaves.
fn finer(c: PyramidChunk, runs: Zoom) -> u32 {
    u32::from(runs - GROUND.cell_zoom(GROUND.read_zoom(c.cell_zoom())))
}

/// `c`, or past the leaves the chunk over it whose runs are its tile's cells.
pub(crate) fn leaf_of(c: PyramidChunk, runs: Zoom) -> PyramidChunk {
    core::iter::once(c).chain(c.ancestors()).nth(finer(c, runs) as usize).or_bug(Errno::ENOTRECOVERABLE)
}

impl Chunk {
    /// The chunk's tile and the apron beside it; past the leaves, its leaf
    /// chunk's, which its sample refines (`ChunkSample::refine`).
    pub(super) fn read(&self, runs: Zoom) -> Option<Read> {
        let leaf = leaf_of(self.at, runs);
        let (own, beside) = ground_of(leaf);
        let (x0, y0) = GROUND.read_at(leaf.column(), leaf.cell_zoom());
        Some(Read {
            own: own.value()?,
            beside,
            first: [x0 as i64, y0 as i64],
        })
    }

    pub(super) fn finer(&self, runs: Zoom) -> u32 {
        finer(self.at, runs)
    }

    pub(super) fn leaf(&self, runs: Zoom) -> PyramidChunk {
        leaf_of(self.at, runs)
    }

    /// Where its first run column lies in its leaf chunk, in its own run columns.
    pub(super) fn in_leaf(&self, runs: Zoom) -> [u32; 2] {
        offset_in(self.at.column().1, finer(self.at, runs)).map(|v| v as u32 * (run_span(self.at, runs) - 2 * APRON))
    }
}
