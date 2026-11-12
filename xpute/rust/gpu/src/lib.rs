// xpute-gpu/lib.rs

//! This crate holds the GPU calls of a core that cannot reach a GPU itself. The
//! core writes WebGPU's calls as records of a stream (`encode`, `stream`) and
//! lends them to its runtime, which runs them on a device; spec/gpu/ gives
//! each op's words and the enumerations they carry, and wit/gpu/ the link's
//! interfaces: the runtime's `gpu` and its device, the core's surfaces.
//! `admit` says whether the runtime takes another frame now. With the
//! `runtime` feature, the crate also holds the runtime for a native device
//! (`runtime`). A program that draws nothing links none of it.

pub mod abi;

#[path = "api.spec.rs"]
pub mod spec;

#[path = "enum.spec.rs"]
pub mod enums;

pub mod admit;
pub mod device;
pub mod encode;
pub mod handle;
pub mod stream;

/// These are the settings of a native GPU runtime that the program chooses; the
/// platform chooses the rest of `runtime::Config` (`runtime::native::config`).
/// A WebAssembly core states them as well. Admission reads the frame latency
/// (`admit::frame`). On WebAssembly nothing reads the chunk size or the slow
/// bound, which a page's runtimes take from the page's program (ts/gpu
/// `GpuConfig`, ts/runtime `Config`).
#[derive(Clone, Copy)]
pub struct CoreConfig {
    /// This is how many frames may be admitted and unfinished on the GPU at
    /// once, and it is at least one.
    pub frame_latency: u32,
    /// This is the size of one chunk that buffer writes are staged through.
    /// Choose it as large as one frame's writes; a larger write is staged
    /// across several chunks.
    pub upload_chunk_bytes: u64,
    /// What takes longer than this many milliseconds is logged, the whole
    /// milliseconds: a record of the stream, as `[gpu] op <op> took <ms> ms`
    /// by the op's number, the stream's run as a whole, as `[gpu] run took
    /// <ms> ms`, a completion's landing in the poll, a read's (`map`) and an
    /// image's (`image`). With none, nothing is timed.
    /// A page's runtimes take the same setting, by the same name, from their
    /// own program (ts/gpu `GpuConfig`, ts/runtime `Config`).
    pub slow_ms: Option<u32>,
}

#[cfg(feature = "runtime")]
pub mod runtime;
