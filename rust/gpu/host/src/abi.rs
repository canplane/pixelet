// pxl-gpu-host/abi.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

pub mod gpu;
pub mod surface;
pub mod vsync;

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] =
    &[("pxl:gpu/gpu", 0x54bd98), ("pxl:gpu/vsync", 0x51cb2a)];

/// The methods of the interfaces the world imports that answer at once,
/// each `(handle, method)`, a function without WIT's `async`, which a
/// runtime refuses to carry to a UI on another thread.
pub const CALLS: &[(xpute_link::Handle, xpute_link::Method)] = &[(0x54bd98, 1)];
