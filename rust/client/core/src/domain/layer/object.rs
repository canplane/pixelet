// pxl-core/domain/layer/object.rs

//! The object layer: the cubes a person places. The store is both the
//! record of the placements and their spatial index, since a placement must
//! ask what is already there. At every zoom, a node of the store's tree holds
//! its children folded into one summary (pxl-spec object/volume.rs).

pub mod asset;
pub mod ext;
pub mod meta;
pub mod repository;
pub mod size;
pub mod store;
