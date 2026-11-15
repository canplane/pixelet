// xpute-gen-targets/lib.rs

//! xpute's generation targets, which the tool that generates its code reads
//! rather than knows: its data specs, the errno table among them. Paths are
//! from xpute's root. Its bindings' targets are the link generator's
//! (xpute-link-gen `TARGETS`).

use xpute_gen::spec::data::DataSpec;

/// xpute's data specs: the link's frame, and the errno table.
pub const DATA_SPECS: [DataSpec; 2] = [
    DataSpec {
        json: "link/spec/frame.json5",
        ts: Some("link/ts/src/frame.spec.ts"),
        rs: Some("link/rust/src/frame.spec.rs"),
        cpp: Some(("cpp/xpute/link/frame.spec.hpp", "xpute::link::frame")),
    },
    DataSpec {
        json: "kit/spec/errno.json5",
        ts: Some("kit/ts/src/status/errno.spec.ts"),
        rs: Some("kit/rust/src/status/errno.spec.rs"),
        cpp: Some(("cpp/xpute/kit/status/errno.spec.hpp", "xpute")),
    },
];
