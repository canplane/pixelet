// xpute-tool/spec/cmd.rs

//! xpute's own protocols, and where each one's code goes.

use xpute_gen::spec::cmd::{outputs_at, RuntimeEnd, Target};
use xpute_gen::spec::Output;

use crate::root;

const TARGETS: [Target; 3] = [
    Target {
        json: "spec/abi/sys.json",
        ts: "ts/runtime/src/abi/sys.spec.ts",
        ts_logical: "@xpute/runtime/abi/sys.spec.ts",
        rs: "rust/core/src/abi/sys.spec.rs",
        rs_logical: "xpute-core/abi/sys.spec.rs",
        number: ("crate::abi::number", "./number.ts"),
        import: Some("crate::io::door"),
        cpp: Some(("cpp/xpute/core/abi/sys.spec.hpp", "xpute::abi::sys")),
        runtime_ts: Some("../mod.ts"),
        path: ("crate::io::fs", "../io/path.ts"),
        runtime: Some(RuntimeEnd {
            rs: "rust/runtime/src/abi/sys.spec.rs",
            rs_logical: "xpute-runtime/abi/sys.spec.rs",
            core: "xpute_core::abi::sys",
            abi: "crate::abi",
        }),
    },
    Target {
        json: "spec/abi/gpu.json",
        ts: "ts/gpu/src/abi.spec.ts",
        ts_logical: "@xpute/gpu/abi.spec.ts",
        rs: "rust/gpu/src/abi.spec.rs",
        rs_logical: "xpute-gpu/abi.spec.rs",
        number: ("xpute_core::abi::number", "@xpute/runtime/abi/number.ts"),
        import: Some("xpute_core::io::door"),
        cpp: Some(("cpp/xpute/gpu/abi.spec.hpp", "xpute::abi::gpu")),
        runtime_ts: Some("@xpute/runtime"),
        path: ("xpute_core::io::fs", "@xpute/runtime/io/path.ts"),
        runtime: Some(RuntimeEnd {
            rs: "rust/gpu/src/runtime/abi.spec.rs",
            rs_logical: "xpute-gpu/runtime/abi.spec.rs",
            core: "crate::abi",
            abi: "xpute_runtime::abi",
        }),
    },
    Target {
        json: "spec/abi/fs.json",
        ts: "ts/runtime/src/abi/fs.spec.ts",
        ts_logical: "@xpute/runtime/abi/fs.spec.ts",
        rs: "rust/core/src/abi/fs.spec.rs",
        rs_logical: "xpute-core/abi/fs.spec.rs",
        number: ("crate::abi::number", "./number.ts"),
        import: Some("crate::io::door"),
        cpp: Some(("cpp/xpute/core/abi/fs.spec.hpp", "xpute::abi::fs")),
        runtime_ts: Some("../mod.ts"),
        path: ("crate::io::fs", "../io/path.ts"),
        runtime: Some(RuntimeEnd {
            rs: "rust/runtime/src/abi/fs.spec.rs",
            rs_logical: "xpute-runtime/abi/fs.spec.rs",
            core: "xpute_core::abi::fs",
            abi: "crate::abi",
        }),
    },
];

pub fn outputs() -> Result<Vec<Output>, String> {
    outputs_at(&root(), &TARGETS)
}
