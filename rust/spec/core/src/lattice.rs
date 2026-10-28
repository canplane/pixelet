// pxl-spec-core/lattice.rs

//! gpu-portable: a layer's lattice, how many cells its tile holds a side,
//! the same at every zoom, and its scale on the camera grid's cells; the
//! zooms and indices of its cells, which the CPU and a GPU read tiles by
//! alike. What places it in the world, a position's cell or a tile's
//! neighbors, is pxl-spec's (space/layer/lattice.rs `Cells`).

use crate::{count, Zoom, FINEST_ZOOM, LEAF_CELL};

use crate::param::tile::{LEAF_ZOOM, TOP_ZOOM};

/// Where a cell's center lies in it, as a share of its side from its low
/// corner: what a cell is painted, sampled and crossed at.
pub const CELL_CENTER: f32 = 0.5;

/// A layer's lattice: its cell is its tile's side over its own count of
/// cells, the same at every zoom; held as the cell's zoom on a leaf. Its
/// scale on the camera grid is `scale_steps` zooms: the grid's cell is that
/// many zooms coarser than its own, so it folds that many of its cells a
/// side, or where negative, finer, so its cell spans that many of the grid's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lattice {
    cell: Zoom,
    scale_steps: i32,
}

impl Lattice {
    /// Finer than a point resolves, or no finer than the leaf tile, holds no tile.
    pub const fn new(cell: Zoom) -> Lattice {
        if cell >= FINEST_ZOOM || cell <= LEAF_ZOOM {
            panic!()
        }
        Lattice {
            cell,
            scale_steps: 0,
        }
    }

    /// Drawn in the camera grid's cells `steps` zooms coarser than its own,
    /// or where negative, finer.
    pub const fn scaled(self, steps: i32) -> Lattice {
        Lattice {
            scale_steps: steps,
            ..self
        }
    }

    // What a layer reads for a node of the view, named by its zoom and column
    // and the cell zoom it is drawn at: the tile whose cells fold into those
    // cells, or past the leaves its leaf, and where the node lies in it.

    /// The zoom of the tile cells at `cell` are read from, held to the pyramid's zooms.
    pub const fn read_zoom(self, cell: Zoom) -> Zoom {
        let z = self.tile_zoom_of_grid(cell);
        (if z < TOP_ZOOM as i32 {
            TOP_ZOOM as i32
        } else if z < LEAF_ZOOM as i32 {
            z
        } else {
            LEAF_ZOOM as i32
        }) as Zoom
    }

    /// Its finest cell's zoom: its cells' on a leaf tile.
    pub const fn cell(self) -> Zoom {
        self.cell
    }

    /// The grid's cell a tile at `zoom` is drawn in.
    pub const fn grid_zoom(self, zoom: Zoom) -> Zoom {
        (self.cell_zoom(zoom) as i32 - self.scale_steps) as Zoom
    }

    /// The tile whose cells fold into the grid's cell `cell`, past the pyramid's ends too.
    pub const fn tile_zoom_of_grid(self, cell: Zoom) -> i32 {
        self.tile_zoom(cell) + self.scale_steps
    }

    /// Its cells' zoom on a tile at `z`.
    pub const fn cell_zoom(self, zoom: Zoom) -> Zoom {
        zoom + self.tile_cell_shamt() as Zoom
    }

    /// The tile zoom whose cells are at `cell_zoom`, past the pyramid's either end alike.
    pub const fn tile_zoom(self, cell_zoom: Zoom) -> i32 {
        cell_zoom as i32 - self.tile_cell_shamt() as i32
    }

    /// `tile_cells` as a shift.
    pub const fn tile_cell_shamt(self) -> u32 {
        self.cell - LEAF_ZOOM
    }

    /// How many of its finest cells stand across one at `z`.
    pub const fn finest_in(self, zoom: Zoom) -> u32 {
        count(self.cell, self.cell_zoom(zoom))
    }

    pub const fn tile_cells(self) -> u32 {
        count(self.cell, LEAF_ZOOM)
    }

    pub const fn cell_index(self, tx: u32, ty: u32) -> u32 {
        ty * self.tile_cells() + tx
    }

    pub const fn in_tile(self, index: u32) -> [u32; 2] {
        let n = self.tile_cells();
        [index % n, index / n]
    }
}

/// The ground's tile is 512 cells a side (param.rs `tile::STEPS`): every length
/// the ground keeps in cells follows it. A grid cell is drawn from its own
/// record: its height its tile's own surface there, its four looks the four a
/// zoom finer's (pxl-spec bake/fold.rs).
pub const GROUND: Lattice = Lattice::new(LEAF_CELL).scaled(0);

/// Cells a ground tile is on a side, at every zoom.
pub const TILE_CELLS: u32 = GROUND.tile_cells();
