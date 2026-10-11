// pxl-kernel/query.rs

//! Queries over what is resident: a layer's value at a point (`at`), the
//! first thing a ray meets (`first_hit`), and a conservative bound for descent
//! (`relief`). A query never fetches or holds anything; it returns an exact
//! answer, a coarser one with the tile it lacked, or nothing and that tile.
//!
//! Answers come from what the chunks draw (`compose`), so picks and footing
//! stand on what is on screen. Whether a ray may skip ground is decided from the index
//! lane's bounds instead, since a coarse drawn surface can sit below a peak.

mod block;
mod decor;
mod placed;
mod ray;
mod scatter;
mod support;
mod surface;
mod world;

pub use block::{runs, Blocks, Column, Composer, Surface};
pub use ray::{Detail, Hit, Ray, Struck};
pub use scatter::grow;
pub use support::{over, under};
pub use world::World;

#[cfg(test)]
#[path = "query.test.rs"]
mod test;
