// xpute-gpu/lib.rs

//! This crate holds the GPU calls of a core that cannot reach a GPU itself. The
//! core writes WebGPU's calls as records of a stream (`encode`), and its
//! runtime runs them on a device; spec/gpu/ gives each op's words and the
//! enumerations they carry. `admit` says whether the runtime takes another
//! frame now. With the `runtime` feature, the crate also holds the runtime for
//! a native device (`runtime`). What the core hears on the GPU's major is
//! `submission`, and a core that takes this GPU is defined by `core!`. The crate
//! stands apart from xpute-kit and
//! xpute-core, so a program that draws nothing links none of it.

#[path = "abi.spec.rs"]
pub mod abi;

#[path = "api.spec.rs"]
pub mod spec;

#[path = "enum.spec.rs"]
pub mod enums;

pub mod admit;
pub mod device;
pub mod encode;
pub mod submission;

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

/// A core of `machine` that takes this GPU, as `xpute_core::core!` defines one
/// without it. `cfg` is its `CoreConfig`, and `ask` the
/// generated `GPU_ASK`. A WebAssembly core admits frames
/// as a page does (`admit::page`), and any other runs `native_core!`. It
/// defines `admit_frame` where it is invoked.
#[macro_export]
macro_rules! core {
    ($machine:expr, $fixed_bytes:expr, $entry:expr, $cfg:expr, $ask:expr $(,)?) => {
        const _: () = $machine.check($fixed_bytes);
        const _: ::xpute_core::entry::Entry = $entry;
        const _: () = assert!(
            $machine.stream.is_some(),
            "a core that takes the GPU has a stream"
        );
        const _: () = {
            let _ = $cfg;
            let _ = $ask;
        };

        ::xpute_core::core!(@wasm $machine, $fixed_bytes, $entry);

        #[cfg(not(target_arch = "wasm32"))]
        $crate::native_core!($machine, $fixed_bytes, $entry, $cfg, $ask);

        /// Whether the GPU's runtime takes a frame now without blocking
        /// (`admit`).
        pub fn admit_frame() -> bool {
            #[cfg(target_arch = "wasm32")]
            return $crate::admit::page::<{ $cfg.frame_latency }>();
            #[cfg(not(target_arch = "wasm32"))]
            return $crate::runtime::native::admit_frame();
        }
    };
}
