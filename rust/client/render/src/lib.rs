// pxl-render/lib.rs

//! pixelet's view: how the model looks when it is drawn, and the device that draws it. The
//! crate knows the model and nothing of the core.

pub mod asset;
pub mod frame;
pub mod gpu;
pub mod look;
pub mod paint;
pub mod param;
pub mod pass;
pub mod ray;
pub mod renderer;
pub mod spec;
