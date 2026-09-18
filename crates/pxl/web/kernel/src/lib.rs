//! pixelet's kernel, in Rust: the world's state and what runs over it — the
//! stores, the fetches, the schedulers, the turn — over the model
//! (pxl-spec), drawn through the view (pxl-render), which knows nothing of
//! it. Nothing here knows about WebAssembly, a browser or a thread — pxl-wasm
//! is the one crate that does — so the same code runs under `cargo test`
//! natively and inside the module the client loads.

pub mod control;
pub mod io;
pub mod layer;
pub mod sched;
pub mod shell;
pub mod sys;
pub mod world;
