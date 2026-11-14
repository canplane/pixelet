// pxl-spec/space/coord.rs

//! Where a thing is and how big it is: a position (`q3`), sizes as zooms, what a position
//! means as a world, and the exact projections to and from it.

pub mod aabb;
pub mod adapter;
pub(crate) mod geoid;
pub mod globe;
pub mod mat4;
pub mod q3;
pub mod v3;
pub mod world;
pub mod zoom;
