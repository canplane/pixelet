// pxl-gpu-link/lib.rs

//! What both of the GPU's sides take: the stream's words, each op's and the
//! enumerations they carry (`spec`, `enums`, from spec/gpu/), the
//! addresses a stream's records hold (`addr`), a handle's slot (`handle`), the one rule
//! a frame is admitted by (`admit`), what a guest asks of its GPU (`GpuAsk`)
//! and the settings its program chooses for the GPU's host (`GpuConfig`), as
//! @pxl/gpu/link has them.

#[path = "api.spec.rs"]
pub mod spec;

#[path = "enum.spec.rs"]
pub mod enums;

pub mod addr;
pub mod admit;
pub mod handle;

/// This is what a guest asks of its GPU, as its program's data spec says, by
/// WebGPU's names: the power preference, the features the device must have,
/// and those it takes where the adapter has them. It crosses no link, so no
/// spec owns it: it is defined in Rust and TypeScript alike
/// (@pxl/gpu `GpuAsk`).
#[derive(Clone, Copy)]
pub struct GpuAsk {
    pub power: &'static str,
    pub required_feats: &'static [&'static str],
    pub optional_feats: &'static [&'static str],
}

/// These are the settings of the GPU's host that the program chooses, the
/// same on a page and a device; a device's host chooses the rest of its
/// config (pxl-gpu-host `Config`, `native::config`). It crosses no link, so
/// no spec owns it: it is defined in Rust and TypeScript alike, the same
/// fields and types (@pxl/gpu `GpuConfig`).
#[derive(Clone, Copy)]
pub struct GpuConfig {
    /// This is how many frames may be admitted and unfinished on the GPU at
    /// once, and it is at least one. The host tells it the guest in the
    /// device's block (spec `DEVICE`), which the guest admits frames by
    /// (pxl-gpu-guest `admit::frame`), and a device's surface queues as
    /// many.
    pub frame_latency: u32,
    /// This is the size of one chunk that buffer writes are staged through.
    /// Choose it as large as one frame's writes; a larger write is staged
    /// across several chunks.
    pub upload_chunk_sz: u32,
}
