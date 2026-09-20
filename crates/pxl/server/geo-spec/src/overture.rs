// pxl-geo-spec/overture.rs

//! What Overture's own values mean here: the category translation, the
//! accessors over its `properties` JSON, its Divisions subtypes and the order
//! they are read in.
//!
//! A module rather than the crate, because a provider is replaceable and this
//! is where that shows. Terrain came off OpenStreetMap in September 2026 and
//! what went with it was the adapters, the policies and the ingest — not the
//! agreement between the server and the bake, which is the crate.

pub mod category;
#[path = "overture/category.spec.rs"]
pub mod category_spec;
pub mod division;
pub mod properties;
