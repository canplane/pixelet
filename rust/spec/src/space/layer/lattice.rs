// pxl-spec/space/layer/lattice.rs

//! wgsl-port: a layer's lattice. A lattice says how many cells a layer's tile holds on a
//! side, which is the same at every zoom, and how its cells scale against the camera grid's.
//! The zooms and indices of the layer's cells follow from it, and the CPU and the GPU read
//! tiles by them alike. Placing a lattice in the world, such as finding a position's cell or a
//! tile's neighbors, and the cells so placed (`Cell`), are `Grid`'s.

use crate::space::camera::grid::{cell_zoom_at, split_dist, CLIP_MARGIN};
use crate::space::coord::q3;
use crate::space::coord::q3::{FRAC_W, Q3};
use crate::space::coord::world;
use crate::space::coord::world::{base_of, idx_of};
use crate::space::coord::zoom::{
    cnt, shamt, units, Zoom, FINEST_ZOOM, LEAF_CELL_ZOOM, LEAF_ZOOM, MIN_ZOOM,
};
use crate::space::hierarchy::tree::{
    ancestor_of, child_idx, child_of, first_descendant_of, off_in, parent_of, Node,
};
use crate::space::layer::quadrant::Quadrant;

/// Where a cell's center lies, as a share of its side from its low corner. A cell is
/// painted, sampled and crossed at this point.
pub const CELL_CENTER: f32 = 0.5;

/// A layer's lattice. Its cell is its tile's side divided by its own count of cells a side,
/// at every zoom, and it is stored as the zoom of the cells on a leaf tile. Its scale on the
/// camera grid is `scale_step` zooms: the grid's cell is that many zooms coarser than the
/// layer's, so a grid cell folds that many zooms of the layer's cells. Where `scale_step` is
/// negative, the grid's cell is that many zooms finer, and a layer cell spans the grid cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lattice {
    leaf_cell_zoom: Zoom,
    scale_step: i32,
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
            scale_step: 0,
        }
    }

    /// The lattice drawn in camera grid cells `scale_step` zooms coarser than its own cells, or
    /// finer where `scale_step` is negative.
    pub const fn scaled(self, scale_step: i32) -> Lattice {
        Lattice { scale_step, ..self }
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
        (self.cell_zoom(zoom) as i32 - self.scale_step) as Zoom
    }

    /// The zoom of the tile whose cells fold into a grid cell at `cell_zoom`, not held to the
    /// pyramid's ends.
    pub const fn tile_zoom_of_grid(self, cell_zoom: Zoom) -> i32 {
        self.tile_zoom(cell_zoom) + self.scale_step
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
/// height is its tile's own surface there, and its four asset ids are those of the four
/// cells a zoom finer (`bake/fold.rs`).
pub const GROUND: Lattice = Lattice::new(LEAF_CELL_ZOOM).scaled(0);

/// The cells on a side of a ground tile, at every zoom.
pub const TILE_CELLS: u32 = GROUND.tile_cells();

/// The coarsest zoom `lattice`'s tiles are read at: where it reads the top's
/// ground cells, the coarsest the camera grid draws. A tile coarser is one no
/// reader asks for, so the bake folds no coarser.
pub const fn coarsest(lattice: Lattice) -> Zoom {
    lattice.read_zoom(GROUND.cell_zoom(MIN_ZOOM))
}

/// How far from the eye the view draws with `zoom` its coarsest:
/// where the grid asks the cell a zoom coarser, which nothing holds, so a ray
/// past it would cross cells finer than its distance asks.
pub const fn draw_dist(lattice: Lattice, zoom: Zoom) -> f64 {
    split_dist(lattice.grid_zoom(zoom - 1))
}
/// Every effect's one reach from the eye: out to where the grid's cells one zoom coarser
/// than the ground's leaf begin, where the casters are drawn whole and a face's bits read the
/// leaf's (pxl-render frame/shader/effect.wgsl `effect_share`, pass/shadow.rs `half_width`).
pub const EFFECT_REACH: f64 = split_dist(GROUND.leaf_cell_zoom() - 1);

/// How far from an observer's foot a demand at `zoom` can lie: where a
/// clip level holds the grid's cell a zoom coarser from (`CLIP_MARGIN`), as
/// far past `draw_dist` as it holds every finer cell past where it is
/// drawn. Bounds the run along the ground too.
pub const fn demand_dist(lattice: Lattice, zoom: Zoom) -> f64 {
    draw_dist(lattice, zoom) * CLIP_MARGIN as f64
}

/// A lattice placed in the world: the tiles and cells a node, a point or an
/// index names.
pub trait Grid {
    fn read_tile(self, node: (Zoom, [i32; 2]), cell_zoom: Zoom) -> Quadrant;
    fn z_at(self, r2: f64) -> Zoom;
    fn cell_of(self, zoom: Zoom, at: &Q3) -> Cell;
    fn cell_at(self, tile: Quadrant, idx: u32) -> Cell;
    fn parent_cell(self, q: usize, idx: u32) -> u32;
    fn child_cells(self, idx: u32) -> (usize, [u32; 4]);
}

impl Grid for Lattice {
    fn read_tile(self, (zoom, column): (Zoom, [i32; 2]), cell_zoom: Zoom) -> Quadrant {
        let z = self.read_zoom(cell_zoom);
        Quadrant::new(z, ancestor_of(column, zoom - z))
    }

    /// The tile whose records fold into the grid's cell at `r2`, a distance
    /// squared; held to the pyramid.
    fn z_at(self, r2: f64) -> Zoom {
        self.read_zoom(cell_zoom_at(r2 as f32))
    }

    /// The cell at `zoom` a point stands in; a point, since a cell may be finer than a unit.
    fn cell_of(self, zoom: Zoom, at: &Q3) -> Cell {
        let zoom = self.cell_zoom(zoom);
        let [x, y, _] = idx_of(zoom, at);
        Cell {
            lattice: self,
            zoom,
            at: [x, y],
        }
    }

    fn cell_at(self, tile: Quadrant, idx: u32) -> Cell {
        xpute_kit::ensure!(idx < self.tile_cells() * self.tile_cells(), ENOTRECOVERABLE);
        let (zoom, at) = tile.node();
        let [x0, y0] = first_descendant_of(at, self.tile_cell_shamt());
        let [tx, ty] = self.in_tile(idx);
        Cell {
            lattice: self,
            zoom: self.cell_zoom(zoom),
            at: [x0 + tx as i32, y0 + ty as i32],
        }
    }

    // A tile's four children form one square of 2n × 2n cells a zoom finer, slot q at (q & 1, q >>
    // 1).

    fn parent_cell(self, q: usize, idx: u32) -> u32 {
        let n = self.tile_cells() as i32;
        let (tile, [tx, ty]) = (child_of([0, 0], q), self.in_tile(idx).map(|v| v as i32));
        let [x, y] = parent_of([tile[0] * n + tx, tile[1] * n + ty]);
        self.cell_idx(x as u32, y as u32)
    }

    /// The child's place and the 2×2 cells, in `child_idx`'s order, a parent cell
    /// folds from: `parent_cell`'s inverse.
    fn child_cells(self, idx: u32) -> (usize, [u32; 4]) {
        let n = self.tile_cells() as i32;
        let [x, y] = child_of(self.in_tile(idx).map(|v| v as i32), 0);
        let [x0, y0] = [(x % n) as u32, (y % n) as u32];
        let cells =
            std::array::from_fn(|q| self.cell_idx(x0 + (q & 1) as u32, y0 + (q >> 1) as u32));
        (child_idx([x / n, y / n]), cells)
    }
}

/// A cell of the pyramid: a node of the one hierarchy at its lattice's cell
/// zoom. Its tile is its ancestor the lattice's shift up, and its place in
/// that tile is where it lies in it; both are read off the node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    lattice: Lattice,
    zoom: Zoom,
    at: [i32; 2],
}

impl Cell {
    pub fn lattice(self) -> Lattice {
        self.lattice
    }

    fn shamt(self) -> u32 {
        self.lattice.tile_cell_shamt()
    }

    pub fn tile(self) -> Quadrant {
        Quadrant::new(self.tile_zoom(), ancestor_of(self.at, self.shamt()))
    }

    /// Its place in its tile, as every lane of the tile numbers it.
    pub fn idx(self) -> u32 {
        let [tx, ty] = self.in_tile();
        self.lattice.cell_idx(tx, ty)
    }

    pub fn tile_zoom(self) -> Zoom {
        self.zoom - self.shamt() as Zoom
    }

    /// Its own zoom.
    pub fn zoom(self) -> Zoom {
        self.zoom
    }

    pub fn node(self) -> Node<2> {
        (self.zoom, self.at)
    }

    pub fn in_tile(self) -> [u32; 2] {
        off_in(self.at, self.shamt()).map(|v| v as u32)
    }

    pub fn base(self) -> Q3 {
        base_of(self.zoom, [self.at[0], self.at[1], 0])
    }

    pub fn mid(self) -> Q3 {
        let base = self.base();
        let half = 1i64 << (FRAC_W as i32 + shamt(self.zoom()) - 1);
        world::at(i64::from(base[0]) + half, i64::from(base[1]) + half, 0)
    }

    pub fn side(self) -> f64 {
        units(self.zoom())
    }

    /// The cell `step_cnt` zooms coarser it lies in.
    pub fn up(self, step_cnt: Zoom) -> Cell {
        Cell {
            zoom: self.zoom - step_cnt,
            at: ancestor_of(self.at, step_cnt),
            ..self
        }
    }

    /// The cell at `zoom` of `lattice` that lies `off` of its sides into
    /// this one, no coarser.
    pub(crate) fn inner_at(self, lattice: Lattice, zoom: Zoom, off: [u32; 2]) -> Cell {
        let zoom = lattice.cell_zoom(zoom);
        let [x, y] = first_descendant_of(self.at, zoom - self.zoom);
        Cell {
            lattice,
            zoom,
            at: [x + off[0] as i32, y + off[1] as i32],
        }
    }

    /// The point every layer stands a thing at: the XY center, half a side above
    /// `ground`, since a zoom's cells are as tall as they are wide.
    pub fn center(self, ground: f64) -> Q3 {
        let at = q3::unpack(&self.mid());
        world::point(at[0], at[1], ground + self.side() / 2.0)
    }
}

#[cfg(test)]
#[path = "lattice.test.rs"]
mod test;
