// pxl-render/gpu.rs

//! The device, and everything that only exists on it: the surfaces drawn
//! into, the texture pack's arrays, the pipelines and passes, the sun's
//! shadow map, and the renderer that walks a draw list through them. This is
//! the one layer that talks to WebGPU, and it decides nothing.

pub mod ao;
pub mod api;
#[path = "gpu/api.spec.rs"]
pub mod api_spec;
pub mod layers;
pub mod paint;
pub mod pass;
pub mod pipeline;
pub mod renderer;
pub mod shadow;
pub mod surface;
