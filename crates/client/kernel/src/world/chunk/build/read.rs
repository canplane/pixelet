// pxl-kernel/world/chunk/build/read.rs

//! What a chunk reads of the terrain: the tile it draws from and the ones beside it.

use pxl_spec::procedural::Frame;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

use pxl_render::mesh::paint::terrain::adapter::{beside_index, Around, TileSource, TileView};
use pxl_render::mesh::run::APRON;


use crate::world::pyramid::chunk::Chunk as PyramidChunk;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::hierarchy::tree::offset_in;
use pxl_spec::space::coord::zoom::{count, Zoom};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use super::*;

/// Where a tile's ground is read from: itself (`n` = 1) or the nearest
/// resident tile over it (`drawn_from`).
#[derive(Clone, Copy)]
pub(super) struct Source {
    pub(super) tile: &'static Terrain,
    pub(super) frame: Frame,
    pub(super) origin: [i64; 2],
    pub(super) n: u32,
}

/// The apron reads each neighbour's tile from what that neighbour is drawn
/// from, so a shared edge is one surface.
#[derive(Clone, Copy)]
pub(super) struct Read {
    pub(super) own: Source,
    pub(super) beside: [Option<Source>; 8],
    pub(super) first: [i64; 2],
}

impl Source {
    /// Found by key, so a tile that is gone never yields another tile's bytes.
    pub(super) fn of(p: Tile) -> Option<Source> {
        let pyramid = Terrain::pyramid();
        let src = pyramid.residency().drawn_from(p)?;
        let tile = pyramid.residency().get(src)?;
        let corner = GROUND.cell_at(p, GROUND.cell_index(0, 0));
        Some(Source {
            tile,
            frame: Frame::of(src),
            origin: corner.in_tile_above(p.zoom - src.zoom),
            n: count(GROUND.cell_zoom(p.zoom), GROUND.cell_zoom(src.zoom)),
        })
    }

    pub(super) fn view(&self) -> TileSource<'static> {
        TileSource {
            tile: Builder::tile_view(self.tile),
            frame: self.frame,
            origin: self.origin,
            n: self.n,
        }
    }
}

impl Read {
    pub(super) fn around(&self) -> Around<'static> {
        Around {
            own: self.own.view(),
            beside: self.beside.map(|b| b.as_ref().map(Source::view)),
        }
    }
}

impl Chunk {
    /// The chunk's tile and the apron beside it; past the leaves, its leaf
    /// chunk's, which its sample refines (`ChunkSample::refine`).
    pub(super) fn read(&self, runs: Zoom) -> Option<Read> {
        let leaf = self.leaf(runs);
        let mut beside = [None; 8];
        for ([qx, qy], p) in GROUND.read_beside(leaf.column(), APRON) {
            beside[beside_index(qx, qy)] = Source::of(p);
        }
        let (x0, y0) = GROUND.read_at(leaf.column());
        Some(Read {
            own: Source::of(GROUND.read_tile(leaf.column()))?,
            beside,
            first: [x0 as i64, y0 as i64],
        })
    }

    /// Zooms runs at `runs` are finer than its tile's cells: none but past the leaves.
    pub(super) fn finer(&self, runs: Zoom) -> u32 {
        u32::from(runs - GROUND.cell_zoom(GROUND.read_zoom(self.at.zoom())))
    }

    /// Itself, or past the leaves the chunk over it whose runs are its tile's cells.
    pub(super) fn leaf(&self, runs: Zoom) -> PyramidChunk {
        core::iter::once(self.at).chain(self.at.ancestors()).nth(self.finer(runs) as usize).or_bug(Errno::ENOTRECOVERABLE)
    }

    /// Where its first run column lies in its leaf chunk, in its own run columns.
    pub(super) fn in_leaf(&self, runs: Zoom) -> [u32; 2] {
        offset_in(self.at.column().1, self.finer(runs)).map(|v| v as u32 * (run_span(self.at, runs) - 2 * APRON))
    }
}

impl Builder {
    pub fn tile_view(tile: &Terrain) -> TileView<'_> {
        TileView {
            cells: &tile.ground.cells,
            ele: &tile.ele,
            decks: &tile.decks,
        }
    }
}
