// pxl-kernel/domain/control.rs

//! What the user does to the world and the view, in one order, each part
//! naming only those below it: the pointer's picks and edits (`edit`) and
//! its gestures and the keys (`input`); navigation, going to a place and
//! following the device; the camera; and under them the device's sensors
//! and where the pivot stands (`support`).

pub mod camera;
pub mod edit;
pub mod input;
pub mod navigation;
pub mod param;
pub mod sensor;
pub mod support;
