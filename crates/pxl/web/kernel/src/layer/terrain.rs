// pxl-kernel/layer/terrain.rs

//! The ground itself: its page as a layer of the pyramid and where one lies,
//! and the map center kept standing on it. Which pages are held is the
//! pyramid's (world/pyramid.rs); what the ground under a point is, the
//! access's (world/access.rs); what its cells paint as is the renderer's
//! (pxl-render mesh/paint/terrain/adapter.rs `MATERIALS`).

pub mod extent;
pub mod fixed;
pub mod ground_snap;
pub mod page;
