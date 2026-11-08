// xpute-gpu/lib.rs

//! This crate holds the GPU calls of a core that cannot reach a GPU itself. The
//! core writes WebGPU's calls as records of a stream (`encode`), and its
//! runtime runs them on a device; spec/gpu/ gives each op's words and the
//! enumerations they carry. `admit` says whether the runtime takes another
//! frame now. With the `runtime` feature, the crate also holds the runtime for
//! a native device (`runtime`). The crate stands apart from xpute-kit and
//! xpute-core, so a program that draws nothing links none of it.

#[path = "api.spec.rs"]
pub mod spec;

#[path = "enum.spec.rs"]
pub mod enums;

pub mod admit;
pub mod device;
pub mod encode;

/// These are the settings of a native GPU runtime that the program chooses; the
/// platform chooses the rest of `runtime::Config` (`runtime::native::config`).
/// A WebAssembly core states them as well. There the page's admission reads
/// the frame latency (`admit::page`), and nothing reads the chunk size, since
/// the browser stages the page's writes itself.
#[derive(Clone, Copy)]
pub struct CoreConfig {
    /// This is how many frames may be admitted and unfinished on the GPU at
    /// once, and it is at least one.
    pub frame_latency: u32,
    /// This is the size of one chunk that buffer writes are staged through.
    /// Choose it as large as one frame's writes; a larger write is staged
    /// across several chunks.
    pub upload_chunk_bytes: u64,
}

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(test)]
#[path = "program.test.rs"]
mod program;
