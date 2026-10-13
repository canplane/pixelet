// pxl-spec/space/layer/tile.rs

//! The pyramid: tiles of a tile grid, from the leaves up to the top, a
//! tile's zoom its size's, so 2^z of them stand around the world. Cell
//! placement, tile membership, folding order and cell centers are decided
//! here only; a layer owns what a cell carries and how it folds.

use super::lattice::{Lattice, GROUND};
use crate::space::coord::aabb::Aabb;
use crate::space::coord::q3::{self, FRAC_W, Q3};
use crate::space::coord::world::{self, around, wrap_index};
use crate::space::coord::zoom::{base_of, shamt, units, Zoom};
use crate::space::hierarchy::morton::{self, key_of, node_of};
use crate::space::hierarchy::tree::{ancestor_of, child_of, first_descendant_of, offset_in, parent_of, region, slot_of, Node};

/// The leaves' zoom: the finest tile any layer is kept in, a page of the
/// world 2^15 round.
pub const LEAF_ZOOM: Zoom = 15;
const _: () = assert!(morton::bits::<2>(LEAF_ZOOM) < 63, "a tile's key fits its subject's word");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tile {
    /// Its size's.
    pub zoom: Zoom,
    /// Its index at its zoom.
    pub at: [i64; 2],
}

impl Tile {
    pub fn new(zoom: Zoom, at: [i64; 2]) -> Tile {
        Tile { zoom, at }
    }

    /// The tile at `z` the leaf `leaf` lies under.
    pub fn over(zoom: Zoom, leaf: [i64; 2]) -> Tile {
        Tile::new(zoom, ancestor_of(leaf, u32::from(LEAF_ZOOM - zoom)))
    }

    #[inline]
    pub fn parent(self) -> Tile {
        Tile::new(self.zoom - 1, parent_of(self.at))
    }

    /// Its 2D key, X wrapped at the seam; its name is the key written
    /// (`morton::write_name`).
    pub fn key(self) -> u128 {
        let [x, y] = self.at;
        key_of::<2>(self.zoom, [wrap_index(self.zoom, x), y])
    }

    /// `key`'s inverse; `None` for a zoom the pyramid does not hold.
    pub fn of_key(key: u128) -> Option<Tile> {
        let (zoom, at) = node_of::<2>(key);
        (GROUND.top()..=LEAF_ZOOM).contains(&zoom).then(|| tile_at(zoom, at))
    }

    /// What a baked tile is stored as, on disk and in the bucket alike.
    pub fn name(self) -> String {
        morton::name::<2>(self.key())
    }

    pub fn of_name(name: &str) -> Option<Tile> {
        Tile::of_key(morton::key_of_name::<2>(name)?)
    }

    /// Its place among its parent's `children`.
    pub fn slot(self) -> usize {
        slot_of(self.at)
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [Tile; 4] {
        std::array::from_fn(|slot| Tile::new(self.zoom + 1, child_of(self.at, slot)))
    }

    pub fn node(self) -> Node<2> {
        (self.zoom, self.at)
    }

    /// Its first leaf.
    pub fn first_leaf(self) -> [i64; 2] {
        first_descendant_of(self.at, u32::from(LEAF_ZOOM - self.zoom))
    }

    pub fn span(self) -> f64 {
        units(self.zoom)
    }

    /// South-west corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let (zoom, [x, y]) = self.node();
        let base = base_of(zoom, [x, y, 0]);
        (q3::to_real(i64::from(base[0])), q3::to_real(i64::from(base[1])), units(zoom))
    }

    /// The tile as a box of the world: its square, every height.
    pub fn region(self) -> Aabb {
        region(self.node())
    }

    /// A demand's subject: its key's high half, which holds every digit and
    /// the end bit of a key no finer than z31.
    pub fn subject(self) -> u64 {
        (self.key() >> 64) as u64
    }

    pub fn of_subject(subject: u64) -> Tile {
        let (zoom, at) = node_of::<2>(u128::from(subject) << 64);
        Tile::new(zoom, at)
    }
}

/// The coarsest tile of a layer made at `leaf` when the source supplies
/// terrain whole from `top`: as many zooms over its leaves.
pub fn top_over(leaf: Zoom, top: Zoom) -> Zoom {
    top.saturating_sub(LEAF_ZOOM - leaf).max(GROUND.top())
}

/// The 4×4 tiles a zoom finer around a tile's children, each at
/// `neighbor_index`; X wraps and a tile past Y's edge has no object.
pub fn neighborhood(p: Tile) -> [Tile; 16] {
    let [x, y] = child_of(p.at, 0);
    std::array::from_fn(|k| {
        let (qx, qy) = ((k % 4) as i64 - 1, (k / 4) as i64 - 1);
        tile_at(p.zoom + 1, [x + qx, y + qy])
    })
}

/// Where the tile (qx, qy) from a parent's first child, each −1 to 2, lies in
/// its `neighborhood`.
pub const fn neighbor_index(qx: i64, qy: i64) -> usize {
    ((qx + 1) + 4 * (qy + 1)) as usize
}

/// The rows of tiles at `z`, south to north; past them is no tile.
pub fn tile_rows(zoom: Zoom) -> std::ops::Range<i64> {
    let n = around(zoom);
    -n / 2..n / 2
}

/// The tile at `z` at (x, y), X wrapped around the world.
pub fn tile_at(zoom: Zoom, [x, y]: [i64; 2]) -> Tile {
    Tile::new(zoom, [wrap_index(zoom, x), y])
}

/// Where a tile's own children sit in its neighborhood, in `slot_of`'s order.
pub const CHILDREN: [usize; 4] = [5, 6, 9, 10];

/// A cell of the pyramid: a node of the one hierarchy at its lattice's cell
/// zoom. Its tile is its ancestor the lattice's shift up, and its place in
/// that tile is where it lies in it; both are read off the node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    pub(super) lattice: Lattice,
    pub(super) zoom: Zoom,
    pub(super) at: [i64; 2],
}

impl Cell {
    pub fn lattice(self) -> Lattice {
        self.lattice
    }

    fn shamt(self) -> u32 {
        self.lattice.tile_cell_shamt()
    }

    pub fn tile(self) -> Tile {
        Tile::new(self.tile_zoom(), ancestor_of(self.at, self.shamt()))
    }

    /// Its place in its tile, as every lane of the tile numbers it.
    pub fn index(self) -> u32 {
        let [tx, ty] = self.in_tile();
        self.lattice.cell_index(tx, ty)
    }

    pub fn tile_zoom(self) -> Zoom {
        self.zoom - self.shamt() as Zoom
    }

    /// Its own.
    pub fn zoom(self) -> Zoom {
        self.zoom
    }

    pub fn node(self) -> Node<2> {
        (self.zoom, self.at)
    }

    pub fn in_tile(self) -> [u32; 2] {
        offset_in(self.at, self.shamt()).map(|v| v as u32)
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

    /// The cell `steps` zooms coarser it lies in.
    pub fn up(self, steps: Zoom) -> Cell {
        Cell {
            zoom: self.zoom - steps,
            at: ancestor_of(self.at, u32::from(steps)),
            ..self
        }
    }

    /// Where `inner`, no coarser and inside this cell, lies in it, in `inner`'s sides.
    pub fn offset_of(self, inner: Cell) -> [u32; 2] {
        offset_in(inner.at, u32::from(inner.zoom - self.zoom)).map(|v| v as u32)
    }

    /// Where this cell lies in the tile `steps` zooms coarser, in its own sides.
    pub fn in_tile_above(self, steps: Zoom) -> [i64; 2] {
        offset_in(self.at, self.shamt() + u32::from(steps))
    }

    /// `offset_of`'s inverse.
    pub fn inner_at(self, lattice: Lattice, zoom: Zoom, offset: [u32; 2]) -> Cell {
        let zoom = lattice.cell_zoom(zoom);
        let [x, y] = first_descendant_of(self.at, u32::from(zoom - self.zoom));
        Cell {
            lattice,
            zoom,
            at: [x + i64::from(offset[0]), y + i64::from(offset[1])],
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
#[path = "tile.test.rs"]
mod test;
