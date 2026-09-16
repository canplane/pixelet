//! pixelet's core, in Rust: the parts of `@pxl/shared` and the render
//! pipeline that compute over typed data and never touch a platform API.
//! Nothing here knows about WebAssembly, a browser or a thread — pxl-wasm
//! is the one crate that does — so the same code runs under `cargo test`
//! natively and inside the module the client loads.

pub mod app;
pub mod bake;
pub mod global;
pub mod host;
pub mod kernel;
/// apps/web/src/lib, inline so the directory is not taken for the crate root.
pub mod lib {
    pub mod env;
}
pub mod mem;
pub mod mem_spec;
pub mod module;
pub mod render;
pub mod space;
pub mod world;
