// pxl-render/gpu.rs

//! Everything that lives on the device. The one layer that talks to WebGPU; it decides nothing.

pub mod ao;
pub mod api;
pub mod atmosphere;
pub mod clouds;
pub mod grown;
pub mod image_map;
pub mod layers;
pub mod pass;
pub mod pipeline;
pub mod renderer;
pub mod sheet;
#[path = "gpu/span.spec.rs"]
pub mod span_spec;
pub mod stars;
pub mod surface;
pub mod text;
