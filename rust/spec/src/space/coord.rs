// pxl-spec/space/coord.rs

//! Where a thing is and how big: a position (`q3`), sizes as zooms, what a
//! position means as a world, and the exact projections to and from it.

pub mod aabb;
pub mod adapter;
#[path = "coord/earth.spec.rs"]
pub mod earth;
#[path = "coord/geoid.spec.rs"]
pub mod geoid;
pub mod globe;
pub mod mat4;
pub mod q3;
pub mod v3;
pub mod world;
pub mod zoom;
