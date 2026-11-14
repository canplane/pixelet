// xpute-gpu-native-runtime/abi.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

pub mod gpu;
pub mod surface;
pub mod vsync;

/// The interfaces the world imports that the host serves under a named
/// handle, each `(name, handle)`, which are mounted before the guest boots.
pub const IMPORTS: &[(&str, xpute_link::Handle)] =
    &[("xpute:gpu/gpu", 0x79d455), ("xpute:gpu/vsync", 0x42eda6)];

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] =
    &[(gpu::HANDLE, gpu::RUN), (vsync::HANDLE, vsync::NEXT)];
