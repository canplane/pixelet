// pxl-core/domain/control.rs

//! What the user does to the world and the view. The parts are layered, and
//! a part names only the parts below it (pxl-tool rule.rs
//! `CONTROL_ORDER` holds the order). At the top are the pointer's picks and
//! edits (`edit`) and its gestures and the keys (`input`). Below them is
//! navigation, which goes to a place or follows the device. Below that is
//! the camera. At the bottom is where the camera's pivot stands (`support`).

pub(crate) mod camera;
pub(crate) mod edit;
pub(crate) mod input;
pub(crate) mod navigation;
pub(crate) mod param;
pub(crate) mod support;
