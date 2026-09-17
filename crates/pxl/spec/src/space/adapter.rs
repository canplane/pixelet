// pxl-spec/space/adapter.rs

//! A module's name is an identifier, so the numbered files are `epsg3857`
//! and `epsg4326`.

#[path = "adapter/3857.rs"]
pub mod epsg3857;
#[path = "adapter/4326.rs"]
pub mod epsg4326;
pub mod yup;
