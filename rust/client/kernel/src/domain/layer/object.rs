// pxl-kernel/domain/layer/object.rs

//! The cubes a person places. The store is both the record and the spatial
//! index, since a placement must ask what is already there; what each node
//! holds at every zoom is its tree's branches, each its children folded
//! (pxl-spec block/volume.rs).

pub mod ext;
pub mod meta;
pub mod preview;
pub mod repository;
pub mod size;
pub mod store;
