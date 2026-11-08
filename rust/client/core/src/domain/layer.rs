// pxl-core/domain/layer.rs

//! The world's contents, in layers. Each layer owns its store and knows
//! nothing of the other layers.

mod answer;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::{coarsest, Lattice};
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM};
use xpute_core::mem::heap::Block;

use crate::sys::io::demand::{Phases, Stream};
use crate::sys::io::failure::Failure;
use tile_cache::TileCache;

pub mod division;
pub mod earth;
pub mod instance;
pub mod object;
pub mod place;
mod query;
pub mod terrain;
pub mod tile_cache;
pub mod upload;

use crate::sys::io::fetch;
use crate::sys::io::source;
pub use answer::Answer;
pub(in crate::domain) use query::Query;

/// Which tiles a layer asks for.
#[derive(Clone, Copy)]
pub enum AskScope {
    /// The floor alone (tile_cache/floor.rs).
    Floor,
    /// One line alone: the tiles over the division layer's pivot
    /// (`division::line`).
    Line,
    /// The floor, a destination's line, and the view's misses with the lines
    /// over them.
    View,
}

pub trait Base: Clone + 'static {
    const STREAM: Stream;

    /// Which of the tile's children have tiles of their own, as the bake
    /// wrote them.
    fn occupancy(&self) -> Occupancy;

    /// How the layer's tiles lie on the camera grid, which says what part of
    /// the grid a tile at each zoom draws.
    const LATTICE: Lattice;

    /// The zoom the layer's tiles are made at. It matches the bake's
    /// `Base::LEAF`.
    const LEAF: Zoom = LEAF_ZOOM;

    /// The coarsest zoom the layer's tiles are read at (pxl-spec lattice.rs
    /// `coarsest`). The bake folds tiles up to this zoom and no further.
    const COARSEST: Zoom = coarsest(Self::LATTICE);

    /// Which tiles the layer asks for. Its tile cache is sized from this.
    const ASK_SCOPE: AskScope = AskScope::View;

    /// The coarsest zoom the layer's own source supplies whole, for a layer
    /// whose source differs from the ground's declared one (sys/io/source.rs
    /// `min_zoom`). The layer's floor is then the whole reach of that zoom
    /// around the eye, so the layer is drawn past where the ground stops.
    const MIN_ZOOM: Option<Zoom> = None;

    /// The coarsest zoom the layer is asked for and drawn at: its own
    /// `MIN_ZOOM`, or else the ground's declared one, but never coarser than
    /// `COARSEST`. None while the layer has no `MIN_ZOOM` and the source is
    /// not declared yet.
    fn min_zoom() -> Option<Zoom> {
        Self::MIN_ZOOM
            .or(source::min_zoom())
            .map(|t| t.max(Self::COARSEST))
    }

    /// The most tiles held hard for a reader that cannot take a coarser
    /// answer, beyond the view and a destination's line (domain/view/demand.rs
    /// `step`).
    const PIN_CAP: u32 = 0;

    fn tile_cache() -> &'static mut TileCache<Self>;

    fn receive(
        tile: Quadrant,
        record: u32,
        pkt: &[u8],
        phases: &mut Phases,
    ) -> Result<Self, Failure>;

    /// Disposes of the packet a tile arrived in, once slot `record` took the
    /// tile or no slot did. The default frees it; a layer whose GPU copies
    /// the tile from its packet keeps the packet instead (`upload::Uploads`).
    fn keep(_record: Option<u32>, block: Block) {
        fetch::release(Some(block));
    }

    /// The tile with nothing in it, which stands for an absent tile at the
    /// layer's top. None for a layer whose absent tiles are answered by the
    /// tile over them.
    fn empty() -> Option<Self> {
        None
    }
}
