// pxl-kernel/layer/object.rs

//! The cubes a person places: where they are, what they are made of, how
//! large they may be, and what the painter is taught to draw them as. The
//! store is both the record and the spatial index, because a placement has
//! to answer "what is already here" before it is allowed.

pub mod collision;
pub mod geometry;
pub mod material;
pub mod size;
pub mod store;
pub mod teach;
