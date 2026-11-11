// pxl-core/lib.rs

//! pixelet's core holds the world's state and runs everything that works on
//! it. `xpute_core::core!` builds it in the form its target takes. On the
//! page it is a WebAssembly module, and the browser draws its frames. On a
//! device it is a library the app links, and xpute's GPU runtime runs beside
//! it and admits each frame. Every turn enters the core through door.rs
//! `run`. No other file of the core knows about WebAssembly, the browser or
//! threads. The only function a device's app calls outside xpute's names is
//! the transport's decoder (sys/unpack.rs, include/pxl_native.h).

pub(crate) mod domain;
pub mod door;
pub(crate) mod perf;
pub(crate) mod pointer;
pub mod sys;
pub(crate) mod turn;
pub(crate) mod ui;

use door::layout::FIXED_BYTES;
use perf::spec::LIMIT;
use pxl_render::api_spec::device;
use sys::manifest::{GPU_ASK, MANIFEST};
use xpute_gpu::CoreConfig;

/// Whether this is a build made for measuring: `PXL_PERF_DRIVE=1` where it is
/// compiled, read as the page's build reads it (ts/client babel.config.ts).
pub const MEASURING: bool = match option_env!("PXL_PERF_DRIVE") {
    Some(v) => matches!(v.as_bytes(), b"1"),
    None => false,
};

/// How a device's GPU runtime paces the renderer's stream, as
/// spec/client/render/api.json `device` sets it. A build made for measuring
/// has it, and the runtime that runs the core's turns, say a record, a run or
/// a turn slower than perf.json's `LIMIT::SLOW_MS`, as the page's worker and
/// its runtime's config have the page's runtimes say.
const GPU: CoreConfig = CoreConfig {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_bytes: device::UPLOAD_CHUNK_BYTES as u64,
    slow_ms: if MEASURING {
        Some(LIMIT::SLOW_MS)
    } else {
        None
    },
};

// The heap starts past the core's fixed ranges (door/layout.rs). The first
// turn opens the front the runtime laid out where the manifest places it.
xpute_gpu::core!(MANIFEST, FIXED_BYTES, door::run, GPU, GPU_ASK);

/// Runs a native test's body on the core's thread, where every turn runs.
/// Each test runs in a process of its own (cargo nextest), since a process
/// boots one core and a test keeps what it puts in a `Global`. The first call
/// boots that process's core: the front's header is laid and the manifest's
/// door opened over it, in an entry whose boot hands a host that receives
/// nothing, drops what is sent and refuses each call.
#[cfg(test)]
pub(crate) fn on_core<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    use std::sync::{Once, OnceLock};

    use xpute_core::entry::{self, BootInfo};
    use xpute_core::host::Host;
    use xpute_core::io::head;
    use xpute_core::mem::Memory;
    use xpute_core::sched::wake::NO_WAKE;
    use xpute_kit::status::errno::Errno;
    use xpute_runtime::native;

    /// The manifest's largest memory, a desktop's, and no turn: a test's body
    /// is what runs on the core's thread.
    fn cfg() -> native::Config {
        let m = MANIFEST;
        native::Config {
            mem_bytes: m
                .mem_bytes
                .iter()
                .copied()
                .max()
                .unwrap_or(m.min_mem_bytes()),
            heap_at: m.heap_at(m.global_base(), FIXED_BYTES),
            stack_bytes: m.thread_stack_bytes as usize,
            thread_name: "pxl-core-test",
            data_end: m.global_base(),
            now: native::now,
            stop_hook: None,
            publish: None,
            slow_ms: None,
        }
    }

    static CORE: OnceLock<native::Core> = OnceLock::new();
    static BOOTED: Once = Once::new();
    let core = *CORE.get_or_init(|| native::boot(&cfg(), |_| -1.0));
    BOOTED.call_once(|| {
        let m = MANIFEST;
        let (base, bytes) = core.mem();
        // SAFETY: the core's memory, reserved for the process's life; its front
        // is the runtime's to write before the core reads it.
        let mem = unsafe { Memory::new(base.as_ptr().expose_provenance(), bytes) };
        // SAFETY: the front's header, which the core has not read yet.
        unsafe { head::write(mem, m.head_at() as usize, &m.head()) };
        let boot = || BootInfo {
            host: Host {
                receive: |_, _| 0,
                send: |_, _, _| {},
                call: |_| Errno::ENOSYS as i32,
            },
            ..CORE.get().expect("the core is booted").boot_info(&cfg())
        };
        core.run_on(move || {
            entry::run(0.0, boot, |_, _| {
                #[expect(
                    clippy::absolute_paths,
                    reason = "the crate's own `door` takes the name"
                )]
                xpute_core::io::door::boot(&MANIFEST);
                NO_WAKE
            })
        })
        .expect("the core boots");
    });
    core.run_on(f).expect("the core has stopped")
}
