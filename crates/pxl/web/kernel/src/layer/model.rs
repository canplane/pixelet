// pxl-kernel/layer/model.rs

//! The models the world draws and the actors that play them: a parsed glTF
//! registered once as the kernel's lanes (`asset`), and every model and
//! placed actor, animated each tick (`store`). Companions, decor kinds and a
//! picker's preview all draw through it.

pub mod asset;
pub mod occupancy;
pub mod store;
