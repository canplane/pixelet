// pxl-kernel/query/surface.rs

//! The ground's surface: a cell's top and the block its face is, read off
//! the windows by the one reading of a column the GPU reads too
//! (pxl-portable `window::ground`), the area's edge laid on it. What a
//! policy over the ground — where something stands, where decor grows,
//! which way it faces — reads, knowing no tile.

use pxl_portable::area::Shard;
use pxl_portable::window::Found;
use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;

use super::world::World;
use crate::layer::terrain::window::Windowed;
use crate::layer::{Answer, Query};

/// The ground's top in a cell, in units, and the block its face is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub top: f64,
    pub block: GroundId,
}

/// A node's ground at its tile's own cells, its own held, a ring past it
/// read as it is: each cell read off the
/// windows as it is asked (`ground`), what is known of the area at the
/// node's shard found once for them all.
pub struct Surfaces {
    cell: Zoom,
    shard: (Shard, (Zoom, [i32; 2])),
}

impl Surfaces {
    /// The cell `g` at its zoom, where it has ground: its face's look and
    /// its top, in blocks.
    pub fn ground(&self, g: [i64; 2]) -> Option<(GroundId, i32)> {
        let (c, _) = pxl_portable::window::ground(&Windowed, self.shard, Found::NONE, self.cell, g.map(|v| v as i32));
        c.grounded().then_some((c.look as GroundId, c.top))
    }

    /// Its cells' zoom.
    pub(super) fn cell(&self) -> Zoom {
        self.cell
    }
}

impl World<'_> {
    /// The surface of the cell `c` of `cell`: its ground's top, in units,
    /// and its face; past the service area the Earth's, at sea level; none
    /// where it has no ground.
    pub fn surface_of(&self, cell: Zoom, c: [i64; 2]) -> Answer<Option<Surface>> {
        let shard = Shard::UNKNOWN.at(&Windowed, (cell, c.map(|v| v as i32)));
        if shard.past() {
            return Answer::Exact(Some(Surface {
                top: f64::from(pxl_portable::SEA_LEVEL),
                block: pxl_spec::block::cube::sentinel::unknown,
            }));
        }
        let line = self.terrain.cursor((cell, c), cell);
        self.terrain.column(&line, shard, Found::NONE, (cell, c)).map(|col| {
            col.grounded().then(|| Surface {
                top: f64::from(col.top) * units(cell),
                block: col.look as GroundId,
            })
        })
    }

    /// The ground over the node `node` of grid cells at `cell`, at the
    /// cells of the tile those cells read as, where the windows hold them
    /// under the node; nothing where they do not. `f` reads it, a cell at
    /// a time, a ring past the node too.
    pub fn surfaces<R>(&self, node: Node<2>, cell: Zoom, f: impl FnOnce(&mut Surfaces) -> R) -> Answer<R> {
        let own = GROUND.cell_zoom(GROUND.read_zoom(cell));
        let at = (node.0, node.1.map(|v| v as i32));
        let shard = (Shard::UNKNOWN.at(&Windowed, at), at);
        self.terrain.cells_held(&self.terrain.cursor(node, cell), node).map(|()| f(&mut Surfaces { cell: own, shard }))
    }
}
