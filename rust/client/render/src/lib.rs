// pxl-render/lib.rs

//! pixelet's view: what the model looks like drawn, and the device that draws
//! it. It knows the model and nothing of the kernel.

/// The protocol's words over xpute's GPU calls (spec/client/render/gpu/api.json):
/// the stream, a pipeline record's codes, a surface's role. Data naming
/// nothing here, so a leaf under both what describes a frame (`frame`) and
/// the device that draws it (`gpu`), which each speak it.
#[path = "gpu/api.spec.rs"]
pub mod api_spec;
pub mod asset;
pub mod frame;
pub mod gpu;
pub mod look;
pub mod mesh;
pub mod param;
