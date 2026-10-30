// pxl-kernel/profiler.rs

//! A read-only instrument for the schedulers; it never feeds back into
//! scheduling. It stands beside the kernel's order and reads anything, and
//! production reaches it only where it records: what each frame and each
//! task cost, in rings laid out by spec/client/kernel/profiler/profiler.json,
//! which the client reads in place (ts/client/src/port/profiler.ts) and the
//! shell prints as a report, beside the place's own long frames
//! (ts/client/src/bracket.tsx `longFrame`), or exports as a Chrome trace.

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
