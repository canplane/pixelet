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

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[
    ("xpute:stop/stop", 0x4eccaa),
    ("xpute:clocks/monotonic-clock", 0x73f703),
    ("xpute:clocks/system-clock", 0x7dfa61),
    ("xpute:fs/file", 0x5b6743),
    ("xpute:gpu/gpu", 0x79d455),
    ("xpute:gpu/vsync", 0x42eda6),
    ("pxl:core/object", 0x4edab0),
    ("pxl:core/overlay", 0x7b1393),
    ("pxl:core/text", 0x61d944),
];

/// The methods of the package's own named interfaces its world imports
/// that answer later, each `(handle, method)`, as WIT's `async func`
/// marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] = &[(object::HANDLE, object::APPLY)];
