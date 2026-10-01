// pxl-kernel/control/camera.rs

//! The camera and everything that moves it: the pointer's gestures, the
//! orbit model they drive, the damping between what was asked and what is
//! drawn, the pick a control needs, and how far one gets before terrain is
//! in the way.

pub mod approach;
pub mod config;
pub mod controller;
pub mod free;
pub mod gesture;
pub mod input;
pub mod orbit;
pub mod ray;
pub mod store;
