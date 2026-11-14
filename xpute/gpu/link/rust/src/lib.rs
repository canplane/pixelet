// xpute-gpu-link/lib.rs

//! What both of the GPU's sides take: the stream's words, each op's and the
//! enumerations they carry (`spec`, `enums`, from gpu/link/spec/), a
//! handle's slot (`handle`), the one rule a frame is admitted by (`admit`),
//! what a guest asks of its GPU (`GpuAsk`) and the settings its program
//! chooses for the GPU's host (`GpuConfig`), as @xpute/gpu-link has them but
//! for the frame latency, which a page's host never reads.

#[path = "api.spec.rs"]
pub mod spec;

#[path = "enum.spec.rs"]
pub mod enums;

pub mod admit;
pub mod handle;

/// This is what a guest asks of its GPU, as its program's data spec says, by
/// WebGPU's names: the power preference, the features the device must have,
/// and those it takes where the adapter has them.
#[derive(Clone, Copy)]
pub struct GpuAsk {
    pub power: &'static str,
    pub required_feats: &'static [&'static str],
    pub optional_feats: &'static [&'static str],
}

/// These are the settings of the GPU's host that the program chooses; a
/// device's host chooses the rest of its config (xpute-gpu-native-host
/// `Config`, `native::config`). The guest admits frames by the frame latency
/// (xpute-gpu-guest `admit::frame`) on every platform; a page's host reads
/// only the chunk size and the slow bound.
#[derive(Clone, Copy)]
pub struct GpuConfig {
    /// This is how many frames may be admitted and unfinished on the GPU at
    /// once, and it is at least one.
    pub frame_latency: u32,
    /// This is the size of one chunk that buffer writes are staged through.
    /// Choose it as large as one frame's writes; a larger write is staged
    /// across several chunks.
    pub upload_chunk_sz: u64,
    /// What takes longer than this many milliseconds is logged, the whole
    /// milliseconds: a record of the stream, as `[gpu] op <op> took <ms> ms`
    /// by the op's number, the stream's run as a whole, as `[gpu] run took
    /// <ms> ms`, a completion's landing in the poll, a read's (`map`) and an
    /// image's (`image`). With none, nothing is timed.
    pub slow_ms: Option<u32>,
}
