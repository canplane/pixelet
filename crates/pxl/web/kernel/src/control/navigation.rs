// pxl-kernel/control/navigation.rs

//! Where the world puts itself: following the device's location and heading,
//! whether the view has been moved off that center, and the fallback that
//! positions the world when a fix genuinely never comes.

pub mod bootstrap;
pub mod destination;
pub mod follow_mode;
pub mod store;
