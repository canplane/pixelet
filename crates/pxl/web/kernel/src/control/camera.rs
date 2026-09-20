// pxl-kernel/control/camera.rs

//! The camera and everything that moves it: the pointer's gestures, the
//! orbit model they drive, the damping between what was asked and what is
//! drawn, the raycast a control needs, and how far one gets before terrain is
//! in the way.

#[path = "camera/config.spec.rs"]
pub mod config;
pub mod damping;
pub mod input;
pub mod orbit;
pub mod ray;
pub mod store;
pub mod terrain_gate;
