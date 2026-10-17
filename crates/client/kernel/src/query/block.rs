// pxl-kernel/query/block.rs

//! The ground a patch of cells reads (`World::ground`: its tile's cells
//! off the windows) and a cell's surface. Neither reads a layer's storage.

use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::block::sample::{Around, Grid, Refused};
use pxl_spec::space::coord::world::wrap_index;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

use super::world::World;
use crate::io::source;
use crate::layer::residency::Line;
use crate::layer::terrain::window::Windowed;
use crate::layer::{Answer, Query};

/// The ground's top in a cell, in units, and the block its face is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub top: f64,
    pub block: GroundId,
}

/// The ground a node's cells are read from (`World::ground`): its own
/// tile's cells, off the windows.
#[derive(Clone, Copy)]
pub struct Ground {
    pub(super) tile: Tile,
    /// The own tile's first cell at `cell`, named on the node's lap round
    /// the world as the node is.
    first: [i64; 2],
    cell: Zoom,
}

impl Ground {
    /// The tile's cells as the grid reads them, off the windows.
    pub(super) fn around(&self) -> Around<Windowed> {
        let zoom = GROUND.cell_zoom(self.tile.zoom);
        Around {
            cells: Windowed,
            first: self.first_at(zoom),
            zoom,
            past: source::past_at,
        }
    }

    /// The own tile's first cell at `zoom`.
    pub(super) fn first_at(&self, zoom: Zoom) -> [i64; 2] {
        let d = zoom as i32 - self.cell as i32;
        self.first.map(|v| if d >= 0 { v << d } else { v >> -d })
    }
}

impl<'a> World<'a> {
    /// The ground the cells of `node` at `cell` are read from: the tile
    /// `line` walked to, where the windows hold its cells under the node.
    pub(super) fn ground(&self, line: &Line, node: (Zoom, [i64; 2]), cell: Zoom) -> Answer<Ground> {
        let tile = line.tile;
        let mut first = tile.at.map(|v| v << (cell - tile.zoom));
        let x = node.1[0] << (cell - node.0);
        first[0] = x - wrap_index(cell, x - first[0]);
        self.terrain.cells_held(line, node).map(|()| Ground { tile, first, cell })
    }

    /// The ground of `node`'s cells at `zoom` and `ring` about them, read off
    /// the ground its cells at `cell` are read from, `line`'s tile, into
    /// `into`; past the service area, where no tile is held, the Earth's.
    /// Nothing where its ground is not held or the heap refused it room.
    pub(super) fn patch_ground(&self, line: &Line, node: (Zoom, [i64; 2]), cell: Zoom, zoom: Zoom, ring: u32, into: &mut Patch) -> Answer<()> {
        into.cell = zoom;
        into.earth = source::past_at(node.1.map(|v| (v as f64 + 0.5) * units(node.0)));
        if into.earth {
            return Answer::Exact(());
        }
        self.ground(line, node, cell).and_then(|g| {
            let d = zoom as i32 - node.0 as i32;
            let (own, across) = match d {
                0.. => (node.1.map(|v| v << d), 1 << d),
                _ => (node.1.map(|v| v >> -d), 1),
            };
            match into.grid.read(&g.around(), zoom, g.tile.zoom, g.first_at(zoom), own.map(|v| v - i64::from(ring)), across + 2 * ring) {
                Ok(()) => Answer::Exact(()),
                Err(Refused) => Answer::Nothing(g.tile),
            }
        })
    }

    /// The surface of the cell `c` of `cell`: its ground's top, in units,
    /// and its face; none where it has no ground.
    pub fn surface_of(&self, cell: Zoom, c: [i64; 2]) -> Answer<Option<Surface>> {
        let patch = PATCH.get(Patch::default);
        self.patch_ground(&self.terrain.cursor((cell, c), cell), (cell, c), cell, cell, 1, patch).map(|()| {
            if patch.earth {
                return Some(Surface {
                    top: f64::from(pxl_portable::SEA_LEVEL),
                    block: pxl_spec::block::cube::sentinel::unknown,
                });
            }
            let g = &patch.grid;
            g.index(c).and_then(|i| {
                g.top(i).map(|t| Surface {
                    top: f64::from(t) * units(cell),
                    block: g.face(i),
                })
            })
        })
    }
}

/// The ground's grid a reader reads its cells into (`World::patch_ground`),
/// kept between asks so a settled ask takes nothing from the heap.
#[derive(Default)]
pub(super) struct Patch {
    grid: Grid,
    cell: Zoom,
    /// Past the service area: the Earth's ground, solid to sea level.
    earth: bool,
}

impl Patch {
    /// Its cells' grid; none past the service area, where no tile is held.
    pub(super) fn grid(&self) -> Option<&Grid> {
        (!self.earth).then_some(&self.grid)
    }

    pub(super) fn cell(&self) -> Zoom {
        self.cell
    }
}

/// The patch kept for a reader.
static PATCH: Global<Patch> = Global::new();
