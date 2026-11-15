// pxl-core/sys.rs

//! The lowest layer of the core, which the domain stands on: its I/O, the
//! mechanics of an entry, its heap and fixed ranges, the files it loads, and
//! the facts of the device's sensors.
//! Nothing here uses the domain, the renderer or the profiler (pxl-tool
//! rule.rs).

pub(crate) mod heap;
#[cfg(test)]
#[path = "sys/host.test.rs"]
pub(crate) mod host;
pub(crate) mod io;
pub(crate) mod load;
pub(crate) mod range;
pub(crate) mod sched;
pub(crate) mod sensor;
