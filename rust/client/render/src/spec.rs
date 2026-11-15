// pxl-render/spec.rs

//! The values the renderer's specs hold, each written from its spec by the
//! generator and read as `crate::spec::<name>`: the words of the protocol over
//! the GPU's calls (`api`, spec/client/render/api.json5), data that names
//! nothing in this crate, so both `frame`, which describes a frame, and `gpu`,
//! the device that draws it, can speak it; and the GPU spans a frame is timed
//! in (`span`, gpu/span.json5). The shaders' looks (debug_look.json5) are the
//! build script's alone.

#[path = "spec/api.spec.rs"]
pub mod api;
#[path = "spec/span.spec.rs"]
pub mod span;
