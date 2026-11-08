// pxl-render/lib.rs

//! pixelet's view: how the model looks when it is drawn, and the device that draws it. The
//! crate knows the model and nothing of the core.

/// The words of the protocol over xpute's GPU calls, generated from
/// spec/client/render/gpu/api.json: the stream, a pipeline record's codes and a surface's
/// role. It is data that names nothing in this crate, so both `frame`, which describes a
/// frame, and `gpu`, the device that draws it, can speak it.
#[path = "api.spec.rs"]
pub mod api_spec;
pub mod asset;
pub mod frame;
pub mod gpu;
pub mod look;
pub mod mesh;
pub mod param;
