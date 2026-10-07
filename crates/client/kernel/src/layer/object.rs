// pxl-kernel/layer/object.rs

//! The cubes a person places. The store is both the record and the spatial
//! index, since a placement must ask what is already there.

pub mod repository;
pub mod size;
pub mod store;
