// pxl-kernel/layer/terrain.rs

//! The ground itself: its page as a layer of the pyramid and where one lies,
//! the elevation sampled under a point, and what the paint is taught about
//! its materials. Which pages are held is the pyramid's (world/pyramid.rs).

pub mod elevation;
pub mod fixed;
pub mod ground_snap;
pub mod page;
pub mod present;
pub mod teach;
