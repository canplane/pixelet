// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it. Nothing here
//! knows of WebAssembly, a browser or threads (the brackets do), so the same
//! code runs under `cargo test` and in either bracket.

pub(crate) mod domain;
pub mod door;
pub(crate) mod perf;
pub(crate) mod pointer;
pub(crate) mod shell;
pub mod sys;
pub(crate) mod turn;
