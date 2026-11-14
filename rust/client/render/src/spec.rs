// pxl-render/spec.rs

//! The values the renderer's specs hold, each written from its spec by the
//! generator and read as `crate::spec::<name>`: the words of the protocol over
//! xpute's GPU calls (`api`, spec/client/render/api.json), data that names
//! nothing in this crate, so both `frame`, which describes a frame, and `gpu`,
//! the device that draws it, can speak it; and the GPU spans a frame is timed
//! in (`span`, gpu/span.json). The shaders' looks (debug_look.json) are the
//! build script's alone.

#[path = "spec/api.spec.rs"]
pub mod api;
#[path = "spec/span.spec.rs"]
pub mod span;
