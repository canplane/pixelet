// pxl-kernel/layer/object.rs

//! The cubes a person places. The store is both the record and the spatial
//! index, since a placement must ask what is already there; the pyramid is
//! what each node holds at every zoom, derived from it.

pub mod ext;
pub mod meta;
pub mod preview;
pub mod pyramid;
pub mod repository;
pub mod size;
pub mod store;
