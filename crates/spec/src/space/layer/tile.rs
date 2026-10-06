// pxl-spec/space/layer/tile.rs

//! The pyramid: tiles of a tile grid, from the leaves up to the top, a
//! tile's zoom its size's, so 2^z of them stand around the world. Cell
//! placement, tile membership, folding order and cell centers are decided
//! here only; a layer owns what a cell carries and how it folds (`Folds`).

use super::lattice::Lattice;
use crate::space::coord::pvf3::core::FRAC_W;
use crate::space::coord::pvf3::ivec::{self, IVec, TO_XY};
use crate::space::coord::pvf3::pack;
use crate::space::coord::pvf3::pvf3::{self, Pvf3};
use crate::space::coord::v3::Aabb;
use crate::space::coord::world::{self, around, wrap_index, REACH};
use crate::space::coord::zoom::{base_of, count, shamt, units, Zoom};
use crate::space::hierarchy::tree::{ancestor_of, child_of, first_descendant_of, parent_of};

/// The leaves' zoom: the finest tile any layer is kept in, a page of the
/// world 2^15 round.
pub const LEAF_ZOOM: Zoom = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tile {
    /// Its size's.
    pub zoom: Zoom,
    /// Z is zero.
    pub tile: IVec,
}

impl Tile {
    pub fn new(zoom: Zoom, tile: IVec) -> Tile {
        Tile { zoom, tile: TO_XY(tile) }
    }

    /// The tile at `z` a leaf lies under.
    pub fn over(zoom: Zoom, leaf: IVec) -> Tile {
        let [x, y] = ancestor_of([ivec::X(leaf), ivec::Y(leaf)], u32::from(LEAF_ZOOM - zoom));
        Tile::new(zoom, ivec::gather(x, y, 0))
    }

    fn at(self) -> [i64; 2] {
        [ivec::X(self.tile), ivec::Y(self.tile)]
    }

    pub fn parent(self) -> Tile {
        let [x, y] = parent_of(self.at());
        Tile::new(self.zoom - 1, ivec::gather(x, y, 0))
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [Tile; 4] {
        std::array::from_fn(|slot| {
            let [x, y] = child_of(self.at(), slot);
            Tile::new(self.zoom + 1, ivec::gather(x, y, 0))
        })
    }

    pub fn node(self) -> (Zoom, [i64; 2]) {
        (self.zoom, self.at())
    }

    /// Its first leaf.
    pub fn first_leaf(self) -> IVec {
        let [x, y] = first_descendant_of(self.at(), u32::from(LEAF_ZOOM - self.zoom));
        ivec::gather(x, y, 0)
    }

    pub fn span(self) -> f64 {
        units(self.zoom)
    }

    /// South-west corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let (zoom, [x, y]) = self.node();
        let base = base_of(zoom, [x, y, 0]);
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), units(zoom))
    }

    /// The tile as a box of the world: its square, every height.
    pub fn region(self) -> Aabb {
        let (x0, y0, side) = self.rect();
        Aabb {
            min: [x0, y0, REACH.min[2]],
            max: [x0 + side, y0 + side, REACH.max[2]],
        }
    }

    /// A demand's subject: the key with the zoom in the Z lane.
    pub fn subject(self) -> u64 {
        ivec::gather(ivec::X(self.tile), ivec::Y(self.tile), self.zoom as i64)
    }

    pub fn of_subject(subject: u64) -> Tile {
        Tile::new(ivec::Z(subject) as Zoom, subject)
    }
}

/// The 4×4 tiles a zoom finer around a tile's children, each at
/// `neighbor_index`; X wraps and a tile past Y's edge has no object.
pub fn neighborhood(p: Tile) -> [Tile; 16] {
    let [x, y] = child_of(p.at(), 0);
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
    Tile::new(zoom, ivec::gather(wrap_index(zoom, x), y, 0))
}

/// Where a tile's own children sit in its neighborhood, in `slot_of`'s order.
pub const CHILDREN: [usize; 4] = [5, 6, 9, 10];

/// A cell of the pyramid: lattice, tile and index. Built only from a point
/// or a tile's own index, so a mismatched zoom and index cannot be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    pub(super) lattice: Lattice,
    pub(super) tile: Tile,
    pub(super) index: u32,
}

impl Cell {
    pub fn lattice(self) -> Lattice {
        self.lattice
    }

    pub fn tile(self) -> Tile {
        self.tile
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn tile_zoom(self) -> Zoom {
        self.tile.zoom
    }

    /// Its own.
    pub fn zoom(self) -> Zoom {
        self.lattice.cell_zoom(self.tile.zoom)
    }

    pub fn in_tile(self) -> [u32; 2] {
        self.lattice.in_tile(self.index)
    }

    pub fn base(self) -> Pvf3 {
        let [tx, ty] = self.in_tile();
        self.lattice.cell_base(self.tile, tx, ty)
    }

    pub fn mid(self) -> Pvf3 {
        let base = self.base();
        let half = 1i64 << (FRAC_W as i32 + shamt(self.zoom()) - 1);
        pvf3::gather(pvf3::X(&base) + half, pvf3::Y(&base) + half, 0)
    }

    pub fn side(self) -> f64 {
        units(self.zoom())
    }

    /// The cell `steps` zooms coarser it lies in.
    pub fn up(self, steps: Zoom) -> Cell {
        self.lattice.cell_of(self.tile.zoom - steps, &self.base())
    }

    /// Where `inner`, no coarser and inside this cell, lies in it, in `inner`'s sides.
    pub fn offset_of(self, inner: Cell) -> [u32; 2] {
        let (outer, inner_base) = (self.base(), inner.base());
        let shift = FRAC_W as i32 + shamt(inner.zoom());
        let lane = |o: i64, i: i64| ((i - o) >> shift) as u32;
        [lane(pvf3::X(&outer), pvf3::X(&inner_base)), lane(pvf3::Y(&outer), pvf3::Y(&inner_base))]
    }

    /// Where this cell lies in the tile `steps` zooms coarser, in its own sides.
    pub fn in_tile_above(self, steps: Zoom) -> [i64; 2] {
        let up = self.up(steps);
        let [ax, ay] = up.in_tile();
        let n = count(self.zoom(), up.zoom()) as i64;
        let [ox, oy] = up.offset_of(self);
        [ax as i64 * n + ox as i64, ay as i64 * n + oy as i64]
    }

    /// `offset_of`'s inverse.
    pub fn inner_at(self, lattice: Lattice, zoom: Zoom, offset: [u32; 2]) -> Cell {
        let base = self.base();
        let shift = FRAC_W as i32 + shamt(lattice.cell_zoom(zoom));
        let at = pvf3::gather(pvf3::X(&base) + ((offset[0] as i64) << shift), pvf3::Y(&base) + ((offset[1] as i64) << shift), 0);
        lattice.cell_of(zoom, &at)
    }

    /// The point every layer stands a thing at: the XY center, half a side above
    /// `ground`, since a zoom's cells are as tall as they are wide.
    pub fn center(self, ground: f64) -> Pvf3 {
        let flat = self.mid();
        let at = pack::unpack(flat.i, flat.f);
        world::point(at[0], at[1], ground + self.side() / 2.0)
    }
}

/// A layer's tile as the pyramid holds it. One fold makes each zoom's tiles
/// from the next finer one's, run by the bake. A parent reads its four
/// children unless `sources` says otherwise.
pub trait Folds: Sized {
    fn sources(p: Tile) -> Vec<Tile> {
        p.children().to_vec()
    }

    /// `p` from its sources; None where a required one is not held.
    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self>
    where
        Self: 'a;

    /// Whether `p` already says all of `child`'s ground, so the tree stops
    /// there and the child is never a tile of its own.
    fn settles(&self, _p: Tile, _child: Tile) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "tile.test.rs"]
mod test;
