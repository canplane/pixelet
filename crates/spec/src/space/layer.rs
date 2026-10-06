// pxl-spec/space/layer.rs

//! How a layer's records map onto the camera grid: its lattice, the pyramid
//! of tiles it is kept in, and the demand an observation makes of them.

pub mod demand;
pub mod lattice;
pub mod read;
pub mod tile;
