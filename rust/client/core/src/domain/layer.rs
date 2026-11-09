// pxl-core/domain/layer.rs

//! The world's contents, in layers. Each layer owns its store and knows
//! nothing of the other layers.

mod answer;
use pxl_spec::bake::tile::Tile;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::sys::io::body::Packet;
use crate::sys::io::demand::PhaseList;
use crate::sys::io::failure::Exit;
use tile_cache::TileCache;

pub mod division;
pub mod earth;
pub mod floor;
pub mod instance;
pub mod object;
pub mod place;
// The query's files stand beside the layers', and it is a child here so that a
// layer's reads, visible within this module alone, are its and no one else's.
#[path = "query.rs"]
pub mod query;
pub mod terrain;
pub mod tile_cache;

use crate::sys::io::source;
pub use answer::Answer;

#[derive(Clone, Copy)]
pub enum AskScope {
    /// The floor alone (layer/floor.rs).
    Floor,
    /// One line alone: the tiles over the division layer's pivot
    /// (`division::line`).
    Line,
    /// The floor, a destination's line, and the view's misses with the lines
    /// over them.
    View,
}

pub trait Base: Tile + Sized + 'static {
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

    /// Decodes the packet the tile arrived in into its own, slot `slot` being
    /// the tile's. The tile owns what it decoded and takes the packet, or
    /// drops it, as its layer decides. An `Exit` of `Refused` waits on what
    /// refused it (sys/io/room.rs).
    fn receive(
        tile: Quadrant,
        slot: u32,
        pkt: Packet,
        phases: &mut PhaseList,
    ) -> Result<Self, Exit>;

    /// The tile with nothing in it, which stands for an absent tile at the
    /// layer's top. None for a layer whose absent tiles are answered by the
    /// tile over them.
    fn empty() -> Option<Self> {
        None
    }
}
