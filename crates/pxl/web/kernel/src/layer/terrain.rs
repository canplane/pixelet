// pxl-kernel/layer/terrain.rs

//! The ground itself: its page as a layer of the pyramid and where one lies,
//! and the elevation sampled under a point. Which pages are held is the
//! pyramid's (world/pyramid.rs); what its cells paint as is the renderer's
//! (pxl-render mesh/paint/terrain/adapter.rs `MATERIALS`).

pub mod elevation;
pub mod fixed;
pub mod ground_snap;
pub mod page;
pub mod relief;
