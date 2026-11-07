// pxl-kernel/domain/scene/marker.rs

//! Markers: what is drawn flat to the camera over the world, a billboard,
//! each from its own design on the one atlas. A place as Carto draws it, a
//! pin someone put up (an object of the marker kind) and the chat's line are each read by their own source
//! and drawn together by one arrangement (`arrangement.rs`).

mod arrangement;
mod atlas;
mod candidate;
mod chat;
mod design;
mod line;
mod pin;
mod place;
mod selection;
mod set;

pub use arrangement::{draw, on_moved, step, tap, MarkerHit};
pub use atlas::atlas;
pub use chat::append_chat_line;
pub use design::text_raster;
