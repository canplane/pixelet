// pxl-kernel/domain/layer.rs

//! What is in the world, in layers. A layer owns its store and knows
//! nothing of its neighbors.

mod answer;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::{coarsest, Lattice};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use xpute_guest::mem::heap::Block;

use crate::sys::io::demand::{Phases, Stream};
use crate::sys::io::failure::Failure;
use tile_cache::TileCache;

pub mod division;
pub mod earth;
pub mod instance;
pub mod kept;
pub mod object;
pub mod place;
mod query;
pub mod terrain;
pub mod tile_cache;

pub use answer::Answer;
pub use query::Query;

/// What a layer asks for of the view.
#[derive(Clone, Copy)]
pub enum AskScope {
    /// The floor alone.
    Floor,
    /// A line alone, the division layer's pivot's (`division::line`).
    Line,
    /// The floor, a destination's line and the view's misses with theirs.
    View,
}

pub trait Base: Clone + 'static {
    const STREAM: Stream;

    /// Which of its children are tiles of their own, as the bake wrote it.
    fn occupancy(&self) -> Occupancy;

    /// Its tiles' cells, which say what of the grid a tile at each zoom draws.
    const LATTICE: Lattice;

    /// The zoom its tiles are made at, as the bake's `Base::LEAF`.
    const LEAF: Zoom = LEAF_ZOOM;

    /// The coarsest zoom its tiles are read at (pxl-spec lattice.rs `coarsest`), which
    /// the bake folds up to and no further.
    const COARSEST: Zoom = coarsest(Self::LATTICE);

    /// What it asks of the view; its tile cache is sized so.
    const ASK_SCOPE: AskScope = AskScope::View;

    /// The coarsest zoom the layer's own source supplies whole, where one
    /// differs from the ground's declared top (sys/io/source.rs `top`). Its floor is
    /// then that zoom's whole reach about the eye: it is drawn past where
    /// the ground stops.
    const TOP: Option<Zoom> = None;

    /// The coarsest zoom it is asked for and drawn at: its own source's
    /// (`TOP`) or else the ground's declared one, never coarser than the
    /// coarsest it is read at (`COARSEST`); none until the source is
    /// declared.
    fn top() -> Option<Zoom> {
        Self::TOP
            .or(crate::sys::io::source::top())
            .map(|t| t.max(Self::COARSEST))
    }

    /// The most tiles a reader that takes no coarser answer pins apart from
    /// the view and a destination's line (pxl-kernel domain/view/demand.rs `step`).
    const PINNED: u32 = 0;

    /// Its records, which it alone holds.
    fn tile_cache() -> &'static mut TileCache<Self>;

    fn receive(tile: Tile, record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Self, Failure>;

    /// The packet a tile came in, once the slot `record` took the tile or
    /// none did: freed, or kept where the GPU takes the tile off it
    /// (`kept::Kept`).
    fn keep(_record: Option<u32>, block: Block) {
        crate::sys::io::fetch::release(Some(block));
    }

    /// A tile with nothing in it, which answers an absent tile at the top.
    fn empty() -> Option<Self> {
        None
    }
}
