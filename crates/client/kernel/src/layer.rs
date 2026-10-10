// pxl-kernel/layer.rs

//! What is in the world, in layers. A layer owns its store and knows
//! nothing of its neighbours.

mod answer;
use pxl_spec::bake::format::Occupancy;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::Lattice;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::io::demand::{Phases, Stream};
use crate::io::failure::Failure;
use residency::Residency;

pub mod division;
pub mod earth;
pub mod instance;
pub mod object;
pub mod place;
mod query;
pub mod residency;
pub mod terrain;

pub use answer::{Answer, Relief};
pub use query::{Bound, Query};

/// What a layer asks for of the view.
#[derive(Clone, Copy)]
pub enum Asks {
    /// The floor alone.
    Floor,
    /// A destination's line alone.
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

    /// What it asks of the view; its residency is sized so.
    const ASKS: Asks = Asks::View;

    /// The coarsest zoom the layer's own source supplies whole, where one
    /// differs from the ground's declared top (`floor::top`). Its floor is
    /// then that zoom's whole reach about the eye: it is drawn past where
    /// the ground stops.
    const TOP: Option<Zoom> = None;

    /// Its records, which it alone holds.
    fn residency() -> &'static mut Residency<Self>;

    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Self, Failure>;

    /// A tile with nothing in it, which answers an absent tile at the top.
    fn empty() -> Option<Self> {
        None
    }
}
