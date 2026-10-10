// pxl-kernel/layer.rs

//! What is in the world, by kind. A layer owns its store and knows nothing
//! of its neighbours.

mod answer;
use pxl_spec::bake::format::Occupancy;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::Lattice;

use crate::io::demand::{Category, Phases};
use crate::io::failure::Failure;
use residency::Residency;

pub mod earth;
pub mod geocode;
pub mod instance;
pub mod object;
pub mod poi;
mod query;
pub mod residency;
pub mod terrain;

pub use answer::{Answer, Ground, Relief};
pub use query::{Bound, Query};

pub trait Layer: Clone + 'static {
    const CATEGORY: Category;

    /// Which of its children are tiles of their own, as the bake wrote it.
    fn occupancy(&self) -> Occupancy;

    /// Its tiles' cells, which say what of the grid a tile at each zoom draws.
    const LATTICE: Lattice;

    /// False for a layer that holds only the floor: it asks for no miss and no
    /// destination, and its residency is sized so.
    const DEMANDS: bool = true;

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
