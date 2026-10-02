// pxl-spec/bake.rs

//! Baked page formats shared by the bake and the kernel, so the two cannot drift.

pub mod constant;
pub mod deck;
pub mod fold;
pub mod format;
pub mod ground;
pub mod model;
pub mod pack;
pub mod place_page;
#[path = "bake/poi_badge.spec.rs"]
pub mod poi_badge;
pub mod poi_page;
#[path = "bake/published.spec.rs"]
pub mod published;
pub mod quadkey;
pub mod surface;
pub mod way;
