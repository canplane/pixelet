// pxl-spec/space/coord.rs

//! Where a thing is and how big: the packed position (PVF3), sizes as zooms,
//! what the lanes mean as a world, and the exact projections to and from it.

pub mod adapter;
#[path = "coord/earth.spec.rs"]
pub mod earth;
pub mod mat4;
pub mod pvf3;
pub mod v3;
pub mod world;
pub mod zoom;
