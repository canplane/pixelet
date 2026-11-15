// xpute-gpu-gen-targets/lib.rs

//! The GPU plugin's generation targets, which the tool that generates its
//! code reads rather than knows: its data specs. Paths are from xpute's root.
//! Its bindings' targets are the link generator's (xpute-link-gen
//! `TARGETS`).

use xpute_gen::spec::data::DataSpec;

/// The plugin's data specs: the GPU's calls and the enumerations they carry.
pub const DATA_SPECS: [DataSpec; 2] = [
    DataSpec {
        json: "gpu/link/spec/api.json5",
        ts: Some("gpu/link/ts/src/api.spec.ts"),
        rs: Some("gpu/link/rust/src/api.spec.rs"),
        cpp: Some(("cpp/xpute/gpu/link/api.spec.hpp", "xpute::gpu")),
    },
    DataSpec {
        json: "gpu/link/spec/enum.json5",
        ts: Some("gpu/link/ts/src/enum.spec.ts"),
        rs: Some("gpu/link/rust/src/enum.spec.rs"),
        cpp: Some(("cpp/xpute/gpu/link/enum.spec.hpp", "xpute::gpu")),
    },
];
