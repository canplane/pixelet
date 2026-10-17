// pxl-kernel/query/surface.rs

//! The ground over a node at once, a ring of its tile's cells around it
//! too, read as the chunks draw it: each cell's top and face, its ways at
//! grade included, from one window read once rather than cell by cell. What
//! a policy over the ground —
//! where decor stands, which way it faces, whether its footing holds —
//! reads, knowing no tile.

use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::global::Global;

use super::block::Window;
use super::world::World;
use crate::layer::Answer;

/// A node's cells and the ring around them, in its tile's cells.
pub struct Surfaces<'w> {
    window: &'w mut Window,
}

impl<'w> Surfaces<'w> {
    /// The cell `g` at its zoom, where it has ground and was read: its
    /// face's look and its top, in blocks.
    pub fn ground(&self, g: [i64; 2]) -> Option<(GroundId, i32)> {
        let grid = self.window.grid()?;
        let i = grid.index(g)?;
        grid.top(i).map(|t| (grid.face(i), t))
    }

    /// Its cells' zoom.
    pub(super) fn cell(&self) -> Zoom {
        self.window.cell()
    }
}

/// The window the ground is read into, kept between asks so a settled ask
/// takes nothing from the heap.
static WINDOW: Global<Window> = Global::new();

impl World<'_> {
    /// The ground over the node `node` of grid cells at `cell`, read off
    /// the tile those cells read as a chunk is, at its own cells, and `ring`
    /// of its cells past it on every side, the tiles beside included
    /// (`World::window_ground`); nothing where it is not held or the heap
    /// refused it room. `f` reads that one window.
    pub fn surfaces<R>(&self, node: Node<2>, cell: Zoom, ring: u32, f: impl FnOnce(&mut Surfaces) -> R) -> Answer<R> {
        // The tile's own cells, which fold nothing.
        let own = GROUND.cell_zoom(GROUND.read_zoom(cell));
        let window = WINDOW.get(Window::default);
        self.window_ground(node, cell, own, ring, window).map(|()| f(&mut Surfaces { window }))
    }
}
