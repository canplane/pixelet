// pxl-core/perf.rs

//! The profiler, a read-only instrument for the schedulers. It stands outside
//! the core's layer order and may read any part of the core, and the rest of
//! the core reaches it only to record. It records what each frame and each
//! task cost, in rings laid out by spec/client/core/perf/perf.json5. The
//! client reads them from the replies that hand them over
//! (ts/client/src/service/core/perf.ts). The UI
//! prints them as a report, beside the long frames the platform itself
//! reports (ts/client/src/service/platform/long-frame.ts), or exports them as a
//! Chrome trace.

pub(crate) mod frame;
pub(crate) mod read;
pub(crate) mod record;
pub(crate) mod report;
pub(crate) mod section;
pub(crate) mod task;
pub(crate) mod window;
pub(crate) mod word;
