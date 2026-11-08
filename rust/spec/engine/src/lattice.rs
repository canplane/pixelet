// pxl-engine/lattice.rs

//! gpu-portable: a layer's lattice. A lattice says how many cells a layer's tile holds on a
//! side, which is the same at every zoom, and how its cells scale against the camera grid's.
//! The zooms and indices of the layer's cells follow from it, and the CPU and the GPU read
//! tiles by them alike. Placing a lattice in the world, such as finding a position's cell or a
//! tile's neighbors, is pxl-spec's (space/layer/lattice.rs `Grid`).

use crate::{cnt, Zoom, FINEST_ZOOM, LEAF_CELL_ZOOM, LEAF_ZOOM, MIN_ZOOM};

/// Where a cell's center lies, as a share of its side from its low corner. A cell is
/// painted, sampled and crossed at this point.
pub const CELL_CENTER: f32 = 0.5;

/// A layer's lattice. Its cell is its tile's side divided by its own count of cells a side,
/// at every zoom, and it is stored as the zoom of the cells on a leaf tile. Its scale on the
/// camera grid is `scale_steps` zooms: the grid's cell is that many zooms coarser than the
/// layer's, so a grid cell folds that many zooms of the layer's cells. Where `scale_steps` is
/// negative, the grid's cell is that many zooms finer, and a layer cell spans the grid cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lattice {
    leaf_cell_zoom: Zoom,
    scale_steps: i32,
}

impl Lattice {
    /// Panics where the leaf cell is as fine as a point resolves, or no finer than the leaf
    /// tile: such a lattice holds no tile.
    pub const fn new(leaf_cell_zoom: Zoom) -> Lattice {
        if leaf_cell_zoom >= FINEST_ZOOM || leaf_cell_zoom <= LEAF_ZOOM {
            panic!()
        }
        Lattice {
            leaf_cell_zoom,
            scale_steps: 0,
        }
    }

    /// The lattice drawn in camera grid cells `steps` zooms coarser than its own cells, or finer
    /// where `steps` is negative.
    pub const fn scaled(self, steps: i32) -> Lattice {
        Lattice {
            scale_steps: steps,
            ..self
        }
    }

    // What a layer reads for a node of the view, given the node's zoom, its column and the cell
    // zoom it is drawn at: the tile whose cells fold into the node's cells (past the leaves, the
    // leaf tile), and where the node lies in that tile.

    /// The zoom of the tile that cells at `cell_zoom` are read from, held to the pyramid's zooms.
    pub const fn read_zoom(self, cell_zoom: Zoom) -> Zoom {
        let z = self.tile_zoom_of_grid(cell_zoom);
        (if z < MIN_ZOOM as i32 {
            MIN_ZOOM as i32
        } else if z < LEAF_ZOOM as i32 {
            z
        } else {
            LEAF_ZOOM as i32
        }) as Zoom
    }

    /// The zoom of the finest cells, those on a leaf tile.
    pub const fn leaf_cell_zoom(self) -> Zoom {
        self.leaf_cell_zoom
    }

    /// The zoom of the camera grid cell that a tile at `zoom` is drawn in.
    pub const fn grid_zoom(self, zoom: Zoom) -> Zoom {
        (self.cell_zoom(zoom) as i32 - self.scale_steps) as Zoom
    }

    /// The zoom of the tile whose cells fold into a grid cell at `cell_zoom`, not held to the
    /// pyramid's ends.
    pub const fn tile_zoom_of_grid(self, cell_zoom: Zoom) -> i32 {
        self.tile_zoom(cell_zoom) + self.scale_steps
    }

    /// The zoom of the cells on a tile at `zoom`.
    pub const fn cell_zoom(self, zoom: Zoom) -> Zoom {
        zoom + self.tile_cell_shamt() as Zoom
    }

    /// The zoom of the tile whose cells are at `cell_zoom`, not held to either end of the
    /// pyramid.
    pub const fn tile_zoom(self, cell_zoom: Zoom) -> i32 {
        cell_zoom as i32 - self.tile_cell_shamt() as i32
    }

    /// The base-2 logarithm of `tile_cells`.
    pub const fn tile_cell_shamt(self) -> u32 {
        self.leaf_cell_zoom - LEAF_ZOOM
    }

    /// How many of the finest cells lie across one cell of a tile at `zoom`.
    pub const fn finest_in(self, zoom: Zoom) -> u32 {
        cnt(self.leaf_cell_zoom, self.cell_zoom(zoom))
    }

    pub const fn tile_cells(self) -> u32 {
        cnt(self.leaf_cell_zoom, LEAF_ZOOM)
    }

    pub const fn cell_idx(self, tx: u32, ty: u32) -> u32 {
        ty * self.tile_cells() + tx
    }

    pub const fn in_tile(self, idx: u32) -> [u32; 2] {
        let n = self.tile_cells();
        [idx % n, idx / n]
    }
}

/// The ground's lattice: 512 cells on a tile's side, the ground's one knob, from which every
/// length the ground keeps in cells follows. A grid cell is drawn from its own record. Its
/// height is its tile's own surface there, and its four palette indices are those of the four
/// cells a zoom finer (pxl-spec bake/fold.rs).
pub const GROUND: Lattice = Lattice::new(LEAF_CELL_ZOOM).scaled(0);

/// The cells on a side of a ground tile, at every zoom.
pub const TILE_CELLS: u32 = GROUND.tile_cells();
