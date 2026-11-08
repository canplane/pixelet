// pxl-render/frame/shader.rs

//! The shaders and the pipelines over them, stated from one source so that a shader's
//! bindings and its pipeline's layout cannot disagree. The crate's build holds every layout
//! to what its shader reads and to WebGPU's floor, and checks the order of the frame's calls.

pub mod access;
pub mod floor;
pub mod group;
pub mod part;
pub mod record;
pub mod sequence;
pub mod word;
