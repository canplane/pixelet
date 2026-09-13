//! pixelet's core, in Rust: the parts of `@pxl/shared` and the render
//! pipeline that compute over typed data and never touch a platform API.
//! Nothing here knows about WebAssembly, a browser or a thread — pxl-wasm
//! is the one crate that does — so the same code runs under `cargo test`
//! natively and inside the module the client loads.
//!
//! Ported a piece at a time from the TypeScript it replaces, and held to
//! byte-for-byte agreement with it until the TypeScript is gone
//! (apps/web/src/render/layer/pipeline/wasm_bake.test.ts).

pub mod buddy;
pub mod mesher;
pub mod run_set;
pub mod scan;
pub mod terrain;
pub mod volume;
pub mod voxel;

mod fdlibm;
mod srgb;
