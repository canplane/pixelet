// pxl-kernel/sched/profiler.rs

//! A read-only instrument for the schedulers; it never feeds back into scheduling.

pub mod block;
pub mod frame;
pub mod log;
pub mod record;
pub mod report;
#[path = "profiler/profiler.spec.rs"]
pub mod spec;
pub mod summary;
pub mod task;
pub mod word;
