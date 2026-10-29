// pxl-kernel/profiler.rs

//! A read-only instrument for the schedulers; it never feeds back into scheduling.

pub mod frame;
pub mod read;
pub mod record;
pub mod report;
pub mod section;
#[path = "profiler/profiler.spec.rs"]
pub mod spec;
pub mod task;
pub mod window;
pub mod word;
