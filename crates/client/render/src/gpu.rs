// pxl-render/gpu.rs

//! Everything that lives on the device. The one layer that talks to WebGPU; it decides nothing.

pub mod ao;
pub mod api;
#[path = "gpu/api.spec.rs"]
pub mod api_spec;
pub mod atmosphere;
pub mod clouds;
pub mod image_map;
pub mod layers;
pub mod pass;
pub mod pipeline;
pub mod renderer;
pub mod shadow;
pub mod sheet;
#[path = "gpu/span.spec.rs"]
pub mod span_spec;
pub mod stars;
pub mod surface;
pub mod text;
