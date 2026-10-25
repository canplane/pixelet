// pxl-kernel/domain/scene/marker.rs

//! Markers: what is drawn flat to the camera over the world, a billboard,
//! each from its own design on the one sheet. A place as Carto draws it, a
//! marker someone put up and the chat's line are each read by their own
//! source and laid out together (`layout.rs`).

pub mod batch;
pub mod chat;
pub mod design;
pub mod layout;
mod line;
mod pin;
mod place;
mod selection;
pub mod set;
pub mod sheet;
