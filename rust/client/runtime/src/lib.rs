// pxl-runtime/lib.rs

//! pixelet's library on a device: the core (pxl-core), linked in as a page
//! instantiates its module, and the runtime the app composes around it, as
//! ts/client/src/runtime/worker.ts is a page's. `start` makes the core's
//! thread and composes xpute's runtime on it in the order it runs: it mounts
//! what the host serves, the stop, the clocks, the text, the settings file and
//! the GPU, then what the UI serves, enters the core, and runs the loop for
//! the core's life. The text and the settings are pixelet's own, served on the
//! host's side as ts/client/src/service serves them on a page (text.rs,
//! file.rs); the UI serves the rest of the files and the objects over the
//! remote, as the page's worker does. Beside them is the decoder the app's
//! fetch calls (unpack.rs).

#![cfg(not(target_arch = "wasm32"))]

/// The bindings of pixelet's interfaces on the host's side, generated from
/// spec/client/core/wit/.
pub mod abi;
mod file;
mod text;
mod unpack;

use std::path::PathBuf;

use pxl_core::spec::manifest::stack;
use pxl_core::spec::perf::limit;
use pxl_core::MEASURING;
use pxl_render::spec::api::device;
use xpute_gpu_link::{GpuAsk, GpuConfig};
use xpute_gpu_native_host::{native, Ask};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;
use xpute_link::Handle;
use xpute_native_host::stop::{self, Stop};
use xpute_native_host::{clock, disk, hook};
use xpute_native_runtime::{event_loop, link, remote};

// The GPU's host under xpute's names, which the platform's GPU module calls
// (xpute_gpu.h, and on Android the Kotlin class `xpute.Gpu`).
xpute_gpu_native_host::native_abi!();

// The runtime and its host under xpute's names over `start`, which xpute's
// module calls (xpute.h, and on Android the Kotlin class `xpute.Runtime`).
xpute_native_host::native_abi!(start);

/// How a device's GPU host paces the renderer's stream, as
/// spec/client/render/api.json `device` sets it. A build made for measuring
/// has the host log a record, a run or an entry slower than perf.json's
/// `limit::SLOW_MS`, as the page's worker has the page's hosts log.
const GPU: GpuConfig = GpuConfig {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_sz: device::UPLOAD_CHUNK_BYTES as u64,
    slow_ms: if MEASURING {
        Some(limit::SLOW_MS)
    } else {
        None
    },
};

/// Starts the core's thread. `named` are the named handles of the interfaces
/// the UI serves (remote.ts `Remote.named`), and `prefixes` those it opens
/// at (remote.ts `Remote.prefixes`); `files_dir` is the directory
/// the platform's storage is kept under, and `cache_dir` the one it keeps the
/// app's caches in; `notify` tells the UI a batch waits (xpute-native-runtime
/// `remote::take`), `report_stop` hears a stop and `report_trap` a trap.
pub fn start(
    named: Vec<Handle>,
    prefixes: Vec<Vec<Token>>,
    files_dir: PathBuf,
    cache_dir: PathBuf,
    notify: fn(),
    report_stop: fn(Stop),
    report_trap: fn(),
) {
    hook();
    let spawned = std::thread::Builder::new()
        .name("xpute core".into())
        .stack_size(stack::THREAD_BYTES as usize)
        .spawn(move || {
            stop::serve(report_stop);
            clock::serve();
            text::serve();
            disk::serve(files_dir);
            file::serve();
            native::serve(
                Ask::of(GpuAsk {
                    power: device::POWER,
                    required_feats: device::REQUIRED_FEATURES,
                    optional_feats: device::OPTIONAL_FEATURES,
                }),
                native::config(GPU),
                Some(cache_dir),
            );
            remote::serve(&named, prefixes, abi::LATER);
            // A trapped core is entered no more, and the UI's methods are
            // answered EPIPE; what boot made due or completed is not left
            // waiting.
            link::init(report_trap);
            link::run_due();
            remote::flush(notify);
            event_loop::run(notify)
        });
    spawned.or_bug(Errno::EAGAIN);
}
