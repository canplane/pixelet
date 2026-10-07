// pxl-kernel/layer/terrain.rs

//! The ground as a layer of the pyramid.

pub mod fixed;
mod query;
pub mod scatter;
pub mod tile;

pub(crate) use query::Drawn;
pub use query::{tile_view, Source};
