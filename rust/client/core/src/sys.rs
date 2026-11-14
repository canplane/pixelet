// pxl-core/sys.rs

//! The lowest layer of the core, which the domain stands on: its I/O, the
//! mechanics of an entry, its heap and fixed ranges, and the files it loads.
//! Nothing here uses the domain, the renderer or the profiler (pxl-tool
//! rule.rs).

pub(crate) mod heap;
pub(crate) mod io;
pub(crate) mod load;
pub(crate) mod range;
pub(crate) mod sched;
