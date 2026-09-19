// pxl-kernel/control.rs

//! What moves the view, and what the device says about where it is: the
//! camera and how it damps, the navigation that follows a location or lets go
//! of it, and the two sensors a browser grants — with the permission each
//! waits on, which is why they are stores and not plain readings.

pub mod camera;
pub mod navigation;
pub mod sensor;
