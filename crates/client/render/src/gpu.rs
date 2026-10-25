// pxl-render/gpu.rs

//! Everything that lives on the device. The one layer that talks to WebGPU; it decides nothing.

pub mod ao;
pub mod api;
pub mod atmosphere;
pub mod block;
pub mod cloud;
pub mod grown;
pub mod image_map;
pub mod pack;
pub mod pass;
pub mod pipeline;
pub mod put;
pub mod read;
pub mod renderer;
pub mod shadow;
pub mod sheet;
#[path = "gpu/span.spec.rs"]
pub mod span_spec;
pub mod star;
pub mod surface;
pub mod text;
