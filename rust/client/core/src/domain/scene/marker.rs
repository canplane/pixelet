// pxl-core/domain/scene/marker.rs

//! Markers: billboards drawn flat to the camera over the world, each from its
//! own design on the one atlas. Three sources feed them: the places, as Carto
//! draws them; the pins people put up, which are objects of the marker kind;
//! and the chat's line. Each source is read on its own, and one arrangement
//! draws them all together (`arrangement.rs`).

mod arrangement;
mod atlas;
mod candidate;
mod chat;
mod design;
mod line;
mod param;
mod pin;
mod place;
mod selection;
mod set;

pub use arrangement::{draw, on_changed, step, tap, MarkerHit};
pub use atlas::atlas;
pub use chat::append_chat_line;
pub use design::text_raster;
