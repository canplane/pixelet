// pxl-kernel/sched/profiler.rs

//! Where the schedulers' time and allocations are recorded, and nothing else:
//! a read-only instrument that never feeds back into scheduling. The window
//! reduced into a block the panel reads, and a live stream for what a panel
//! cannot show — a stutter as it happens.

pub mod block;
pub mod log;
pub mod record;
#[path = "profiler/profiler.spec.rs"]
pub mod spec;
pub mod summary;
