// pxl-kernel/profiler.rs

//! A read-only instrument for the schedulers; it never feeds back into scheduling.

pub mod frame;
pub mod record;
pub mod report;
pub mod section;
#[path = "profiler/profiler.spec.rs"]
pub mod spec;
pub mod summary;
pub mod task;
pub mod window;
pub mod word;
