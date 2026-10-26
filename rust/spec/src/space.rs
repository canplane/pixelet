// pxl-spec/space.rs

//! Positions and their structures, each layer reading only those under it:
//! coordinates and sizes, the hierarchy over them, the camera's grid, and
//! how a layer's tiles map onto it.

pub mod camera;
pub mod coord;
pub mod hierarchy;
pub mod layer;
