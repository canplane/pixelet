// pxl-kernel/layer.rs

//! What is in the world, by kind. A layer owns its store, says what it puts
//! in a frame, and knows nothing of its neighbours — terrain and its decor,
//! the objects a person places, the POIs and their badges, the characters,
//! the markers and the billboards. One draws nothing: the place the map
//! centre is in, which the UI snapshot names.

pub mod billboard;
pub mod character;
pub mod decor;
pub mod environment;
pub mod geocode;
pub mod marker;
pub mod object;
pub mod poi;
pub mod terrain;
