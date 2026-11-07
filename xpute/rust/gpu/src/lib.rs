// xpute-gpu/lib.rs

//! The calls a core makes to a GPU it cannot reach, as WebGPU names them:
//! the ops of a stream's records and the enumerations their words carry
//! (spec/gpu/), and whether its runtime admits a frame now (`admit`). Beside
//! `kit` and the core's end: a program that draws nothing links none of it.

#[path = "api.spec.rs"]
pub mod spec;

#[path = "enum.spec.rs"]
pub mod enums;

pub mod admit;
pub mod device;
pub mod encode;

/// What a native GPU runtime runs a core's stream by that is the program's to
/// choose; the rest of its `runtime::Config` is the platform's
/// (`runtime::native::config`). A WebAssembly core names them too and leaves
/// them unread, its page's runtime being the browser's.
#[derive(Clone, Copy)]
pub struct Numbers {
    /// The frames admitted and unfinished at once (`runtime::Config`), at
    /// least one.
    pub frame_latency: u32,
    /// The staging belt's chunk: as large as a frame's writes.
    pub upload_chunk_bytes: u64,
}

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(test)]
#[path = "programs.test.rs"]
mod programs;
