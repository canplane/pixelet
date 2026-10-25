// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it. Nothing here
//! knows of WebAssembly, a browser or threads (the brackets do), so the same
//! code runs under `cargo test` and in either bracket.

pub mod domain;
pub mod door;
pub mod profiler;
pub mod shell;
pub mod sys;
pub mod turn;
