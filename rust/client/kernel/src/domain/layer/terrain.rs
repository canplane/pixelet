// pxl-kernel/domain/layer/terrain.rs

//! The ground as a layer of the pyramid.

pub mod clipmap;
pub mod fixed;
pub mod pool;
mod query;
pub mod tile;

pub(in crate::domain) use query::TerrainQuery;
