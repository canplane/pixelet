// pxl-render/gpu.rs

//! Everything that lives on the device. This is the one layer that talks to WebGPU, and it
//! decides nothing.

pub mod ao;
pub mod api;
pub mod atlas;
pub mod atmosphere;
pub mod cloud;
pub mod draw;
pub mod finish;
pub mod generator;
pub mod image_map;
pub mod mip;
pub mod pack;
pub mod pass;
pub mod pipeline;
pub mod propagate;
pub mod put;
pub mod ray;
pub mod read;
pub mod renderer;
pub mod resolve;
pub mod shadow;
pub mod sky;
#[path = "gpu/span.spec.rs"]
pub mod span_spec;
pub mod split;
pub mod star;
pub mod surface;
pub mod text;
pub mod tile;
pub mod upload;
