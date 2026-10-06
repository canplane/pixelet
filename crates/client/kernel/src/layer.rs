// pxl-kernel/layer.rs

//! What is in the world, by kind. A layer owns its store and knows nothing
//! of its neighbours.

mod answer;
pub mod earth;
pub mod geocode;
pub mod object;
pub mod poi;
mod query;
pub mod residency;
pub mod terrain;

pub use answer::{Answer, Ground, Relief};
pub use query::{Bound, Query};
