// pxl-render/gpu.rs

//! The device layer: WebGPU's calls and what is made directly with them, the draw lists, the
//! pipelines, the surfaces, the textures and tiles. The rays (ray.rs) and the passes (pass.rs)
//! stand on it, the renderer (renderer.rs) over them, and it decides nothing.

pub mod api;
pub mod atlas;
pub mod draw;
pub mod generator;
pub mod image_map;
pub mod mip;
pub mod pack;
pub mod pipeline;
pub mod put;
pub mod read;
#[path = "gpu/span.spec.rs"]
pub mod span_spec;
pub mod split;
pub mod surface;
pub mod text;
pub mod tile;
pub mod upload;
