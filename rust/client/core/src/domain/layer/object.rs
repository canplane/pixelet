// pxl-core/domain/layer/object.rs

//! The object layer: what a person places. The store is both the
//! record of the placements and their spatial index, since a placement must
//! ask what is already there. At every zoom, a node of the store's tree holds
//! its children folded into one summary (volume.rs).

pub(crate) mod asset;
pub(crate) mod ext;
pub(crate) mod meta;
pub(crate) mod repository;
pub(crate) mod size;
pub(crate) mod sparse_tree;
pub(crate) mod store;
pub(crate) mod volume;
