// pxl-render/frame.rs

//! One frame, from what the kernel decided to what the device is told: the
//! uniforms it hands over, the lists of what to draw, and the shaders those
//! lists are drawn with. Nothing here decides — the kernel hands a frame what
//! it reads, and this turns it into work.

pub mod draw;
pub mod shader;
pub mod stage;
