// pxl-kernel/query.rs

//! Queries over what is resident: a layer's value at a point (`at`), the
//! first thing a ray meets (`first_hit`), and a conservative bound for descent
//! (`relief`). A query never fetches or holds anything; it returns an exact
//! answer, a coarser one with the tile it lacked, or nothing and that tile.
//!
//! Answers come from the drawn surface (`ground_ele_at`), so picks stand on
//! what is on screen. Whether a ray may skip ground is decided from the index
//! lane's bounds instead, since a coarse drawn surface can sit below a peak.

mod column;
mod decor;
mod ground;
mod ray;
mod support;
mod world;

pub use ray::{Detail, Hit, Ray, Struck};
pub use support::over;
pub use world::Composed;
pub use world::{covers, Cover, World};

#[cfg(test)]
#[path = "query.test.rs"]
mod test;
