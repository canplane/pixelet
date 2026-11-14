// xpute-gpu-gen-targets/lib.rs

//! The GPU plugin's generation targets, which the tools that generate code
//! read rather than know: its bindings on the guest's side and on the native
//! host's, and its data specs. Paths are from xpute's root.

use xpute_gen::spec::data::DataSpec;
use xpute_link_gen::wit::{Side, WitTarget};

/// The plugin's bindings on the guest's side, with the TypeScript in the
/// plugin's link and the C++ beside its GPU calls.
pub const GUEST_TARGET: WitTarget<'static> = WitTarget {
    wit: "gpu/wit",
    rs: Some("gpu/guest/rust/src/abi.rs"),
    ts: Some("gpu/link/ts/src/abi"),
    cpp: Some("cpp/xpute/gpu/abi"),
    side: Some(Side::Guest("guest/rust")),
};

/// The plugin's bindings on a native device's host's side, served on
/// xpute-native-runtime's link.
pub const NATIVE_HOST_TARGET: WitTarget<'static> = WitTarget {
    wit: "gpu/wit",
    rs: Some("gpu/native/host/src/abi.rs"),
    ts: None,
    cpp: None,
    side: Some(Side::Host("native/runtime/rust")),
};

/// The plugin's data specs: the GPU's calls and the enumerations they carry.
pub const DATA_SPECS: [DataSpec; 2] = [
    DataSpec {
        json: "gpu/link/spec/api.json",
        ts: Some("gpu/link/ts/src/api.spec.ts"),
        rs: Some("gpu/link/rust/src/api.spec.rs"),
        cpp: Some(("cpp/xpute/gpu/api.spec.hpp", "xpute::gpu")),
    },
    DataSpec {
        json: "gpu/link/spec/enum.json",
        ts: Some("gpu/link/ts/src/enum.spec.ts"),
        rs: Some("gpu/link/rust/src/enum.spec.rs"),
        cpp: Some(("cpp/xpute/gpu/enum.spec.hpp", "xpute::gpu")),
    },
];
