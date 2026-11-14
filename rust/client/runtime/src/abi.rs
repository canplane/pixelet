// pxl-runtime/abi.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

pub mod camera;
pub mod edit;
pub mod input;
pub mod object;
pub mod overlay;
pub mod perf;
pub mod scene;
pub mod sensor;
pub mod setting;
pub mod source;
pub mod text;
pub mod types;
pub mod watch;

use xpute_gpu_native_runtime::abi::gpu;
use xpute_gpu_native_runtime::abi::vsync;
use xpute_native_runtime::abi::clocks::monotonic_clock;
use xpute_native_runtime::abi::clocks::system_clock;
use xpute_native_runtime::abi::fs::file;
use xpute_native_runtime::abi::stop;

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[
    ("xpute:stop/stop", stop::HANDLE),
    ("xpute:clocks/monotonic-clock", monotonic_clock::HANDLE),
    ("xpute:clocks/system-clock", system_clock::HANDLE),
    ("xpute:fs/file", file::HANDLE),
    ("xpute:gpu/gpu", gpu::HANDLE),
    ("xpute:gpu/vsync", vsync::HANDLE),
    ("pxl:core/object", object::HANDLE),
    ("pxl:core/overlay", overlay::HANDLE),
    ("pxl:core/watch", watch::HANDLE),
    ("pxl:core/text", text::HANDLE),
];

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] = &[(object::HANDLE, object::APPLY)];
