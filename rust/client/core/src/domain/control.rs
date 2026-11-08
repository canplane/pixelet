// pxl-core/domain/control.rs

//! What the user does to the world and the view. The parts are layered, and
//! a part names only the parts below it (pxl-tool check_rules.rs
//! `CONTROL_ORDER` holds the order). At the top are the pointer's picks and
//! edits (`edit`) and its gestures and the keys (`input`). Below them is
//! navigation, which goes to a place or follows the device. Below that is
//! the camera. At the bottom are the device's sensors (`sensor`) and where
//! the camera's pivot stands (`support`).

pub mod camera;
pub mod edit;
pub mod input;
pub mod navigation;
pub mod param;
pub mod sensor;
pub mod support;
