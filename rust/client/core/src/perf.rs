// pxl-core/perf.rs

//! A read-only instrument for the schedulers. It stands beside the core's
//! order and reads anything, and production reaches it only where it records: what each frame and each
//! task cost, in rings laid out by spec/client/core/perf/perf.json,
//! which the client reads in place (ts/client/src/port/perf.ts) and the
//! UI prints as a report, beside the place's own long frames
//! (ts/client/src/bracket.tsx `longFrame`), or exports as a Chrome trace.

pub mod frame;
pub mod read;
pub mod record;
pub mod report;
pub mod section;
#[path = "perf/perf.spec.rs"]
pub mod spec;
pub mod task;
pub mod window;
pub mod word;
