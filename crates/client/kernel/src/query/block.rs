// pxl-kernel/query/block.rs

//! The ground a block's cell reads (`World::ground`: its tile and those beside
//! it) and a cell's surface, for the world's one query (`World::block`,
//! node.rs). Neither reads a layer's storage.

use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::block::sample::{beside_index, Around, Grid, Refused};
use pxl_spec::space::coord::world::wrap_index;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

use super::world::World;
use crate::io::source;
use crate::layer::residency::Line;
use crate::layer::terrain::window::Windowed;
use crate::layer::terrain::Source;
use crate::layer::{Answer, Query};

/// The ground's top in a cell, in units, and the block its face is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub top: f64,
    pub block: GroundId,
}

/// The ground a node's cells are read from (`World::ground`), which a
/// build is keyed by: its own tile's source and those beside it.
#[derive(Clone, Copy)]
pub struct Ground<'a> {
    pub own: Source<'a>,
    pub beside: [Option<Source<'a>>; 8],
    pub(super) tile: Tile,
    /// The own tile's first cell at `cell`, named on the node's lap round
    /// the world as the node is.
    first: [i64; 2],
    cell: Zoom,
}

impl<'a> Ground<'a> {
    /// The tiles as the one rule reads them, their cells off the windows.
    pub(super) fn around(&self) -> Around<'a, Windowed> {
        let zoom = GROUND.cell_zoom(self.tile.zoom);
        Around {
            cells: Windowed,
            first: self.first_at(zoom),
            zoom,
            own: self.own.view(),
            beside: self.beside.map(|b| b.map(|s| s.view())),
            past: source::past_at,
        }
    }

    /// The own tile's first cell at `zoom`.
    pub(super) fn first_at(&self, zoom: Zoom) -> [i64; 2] {
        let d = zoom as i32 - self.cell as i32;
        self.first.map(|v| if d >= 0 { v << d } else { v >> -d })
    }
}

/// Drawn tile cells a grid cell at `cell` spans upright and across, as a
/// shift: past the leaves none.
fn folds(cell: Zoom) -> u32 {
    let tile = GROUND.read_zoom(cell);
    GROUND.cell_zoom(tile).saturating_sub(cell)
}

impl<'a> World<'a> {
    /// The ground the cells of `node` at `cell` are read from, a ring of
    /// `ring` cells about them included: its own tile, the one `line` walked
    /// to, read as a chunk reads it, and the tiles beside it the ring reaches.
    pub(super) fn ground(&self, line: &Line, node: (Zoom, [i64; 2]), cell: Zoom, ring: u32) -> Answer<Ground<'a>> {
        let mut beside = [None; 8];
        for ([qx, qy], p) in GROUND.read_beside(node, cell, ring << folds(cell)) {
            // A tile is read at its own cells.
            beside[beside_index(qx, qy)] = self.terrain.read((p.zoom, [p.at[0], p.at[1], 0]), GROUND.grid_zoom(p.zoom)).value();
        }
        let tile = line.tile;
        let mut first = tile.at.map(|v| v << (cell - tile.zoom));
        let x = node.1[0] << (cell - node.0);
        first[0] = x - wrap_index(cell, x - first[0]);
        let held = self.terrain.cells_held(line, node);
        self.terrain.cells_of(line).and_then(|own| held.map(|()| Ground { own, beside, tile, first, cell }))
    }

    /// The ground of `node`'s cells at `zoom` and `ring` about them, read off
    /// the ground its cells at `cell` are read from, into `into`; past the
    /// service area, where no tile is held, the Earth's. The own tile's first
    /// cell at `zoom`; nothing where its ground is not held or the heap
    /// refused it room.
    pub(super) fn window_ground(&self, node: (Zoom, [i64; 2]), cell: Zoom, zoom: Zoom, ring: u32, into: &mut Window) -> Answer<[i64; 2]> {
        into.cell = zoom;
        into.earth = source::past_at(node.1.map(|v| (v as f64 + 0.5) * units(node.0)));
        if into.earth {
            return Answer::Exact([0; 2]);
        }
        self.ground(&self.terrain.cursor(node, cell), node, cell, ring).and_then(|g| {
            let d = zoom as i32 - node.0 as i32;
            let (own, across) = match d {
                0.. => (node.1.map(|v| v << d), 1 << d),
                _ => (node.1.map(|v| v >> -d), 1),
            };
            let first = g.first_at(zoom);
            match into.grid.read(&g.around(), zoom, g.tile.zoom, first, own.map(|v| v - i64::from(ring)), across + 2 * ring) {
                Ok(()) => Answer::Exact(first),
                Err(Refused) => Answer::Nothing(g.tile),
            }
        })
    }

    /// The surface of the cell `c` of `cell`: its ground's top, in units,
    /// and its face; none where it has no ground.
    pub fn surface_of(&self, cell: Zoom, c: [i64; 2]) -> Answer<Option<Surface>> {
        let window = WINDOW.get(Window::default);
        self.window_ground((cell, c), cell, cell, 1, window).map(|_| {
            if window.earth {
                return Some(Surface {
                    top: f64::from(pxl_portable::SEA_LEVEL),
                    block: pxl_spec::block::cube::sentinel::unknown,
                });
            }
            let g = &window.grid;
            g.index(c).and_then(|i| {
                g.top(i).map(|t| Surface {
                    top: f64::from(t) * units(cell),
                    block: g.face(i),
                })
            })
        })
    }
}

/// The ground's grid a reader reads its cells into (`World::window_ground`),
/// kept between asks so a settled ask takes nothing from the heap.
#[derive(Default)]
pub(super) struct Window {
    grid: Grid,
    cell: Zoom,
    /// Past the service area: the Earth's ground, solid to sea level.
    earth: bool,
}

impl Window {
    /// Its cells' grid; none past the service area, where no tile is held.
    pub(super) fn grid(&self) -> Option<&Grid> {
        (!self.earth).then_some(&self.grid)
    }

    pub(super) fn cell(&self) -> Zoom {
        self.cell
    }
}

/// The window kept for a reader.
static WINDOW: Global<Window> = Global::new();
