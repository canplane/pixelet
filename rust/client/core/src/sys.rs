// pxl-core/sys.rs

//! The lowest layer of the core, which the domain stands on: its I/O, the
//! mechanics of an entry, its heap and fixed ranges, the files it loads, its
//! manifest of what it asks of its runtime, and the device transport's
//! decoder. Nothing here uses the domain, the renderer or
//! the profiler (pxl-tool rule.rs).

pub(crate) mod heap;
pub(crate) mod io;
pub(crate) mod load;
pub(crate) mod range;
pub(crate) mod sched;
#[cfg(not(target_arch = "wasm32"))]
mod unpack;
