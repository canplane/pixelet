// pxl-geo-bake/terrain/pipeline/overture.rs

//! Overture's way into the page: the adapter over its base and building
//! rows, its linear referencing, the policy that decides what each source
//! paints, and how wide a way is.

pub mod adapter;
pub mod between;
pub mod policy;
pub mod width;
