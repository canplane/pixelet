// pxl-core/sys.rs

//! The lowest layer of the core, which the domain stands on: its I/O, the
//! mechanics of a turn, the sections of its memory, the files it loads, what
//! it was booted with, its manifest of what it asks of its runtime, and the
//! device transport's decoder. Nothing here uses the domain, the renderer or
//! the profiler (pxl-tool rule.rs).

pub(crate) mod boot;
pub(crate) mod io;
/// The roots of the core's files (sys/io/file.json), which a binding answers
/// every path by.
pub use io::file_spec;
pub(crate) mod load;
#[path = "sys/manifest.spec.rs"]
pub mod manifest;
pub(crate) mod sched;
pub(crate) mod section;
#[cfg(not(target_arch = "wasm32"))]
mod unpack;
