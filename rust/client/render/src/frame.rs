// pxl-render/frame.rs

//! What a frame is made of, stated without naming a device: the shaders, their pipeline
//! records and the uniforms they read, the order of the frame's calls over the buffers they
//! share (`shader`), and where each marker's bitmap lies in the atlas (`shelf`). The code
//! under `gpu` turns these into device calls.

pub mod shader;
pub mod shelf;
