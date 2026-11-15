// pxl-tool/spec/cmd.rs

//! The link's bindings of pixelet's packages, written by xpute-link-gen as
//! any program's are, over xpute's own packages, which it carries itself:
//! the GPU's (spec/gpu/wit/), the plugin the core takes, and the
//! core's (spec/client/core/wit/) over it. The guest's Rust is in
//! pxl-gpu-guest and pxl-core; a host's Rust in pxl-gpu-host and in
//! pxl-runtime, which serves pixelet's own interfaces
//! (rust/client/runtime/src/text.rs) on xpute-host's link, and its
//! TypeScript in @pxl/gpu/link, which both of the GPU's UI ends and its web
//! host take, and in the client.

use xpute_link_gen::wit::{Side, WitTarget, wit_outputs};

use super::Output;
use crate::root;

const GPU: &str = "spec/gpu/wit";
const CORE: &str = "spec/client/core/wit";

/// The GPU's bindings on each side, which the core's take.
const GPU_TARGETS: [WitTarget<'static>; 2] = [
    WitTarget {
        wit: GPU,
        rs: Some("rust/gpu/guest/src/abi.rs"),
        ts: None,
        cpp: None,
        side: Some(Side::Guest),
    },
    WitTarget {
        wit: GPU,
        rs: Some("rust/gpu/host/src/abi.rs"),
        ts: Some("ts/gpu/link/src/abi"),
        cpp: None,
        side: Some(Side::Host),
    },
];

/// The core's bindings on each side.
const CORE_TARGETS: [WitTarget<'static>; 2] = [
    WitTarget {
        wit: CORE,
        rs: Some("rust/client/core/src/abi.rs"),
        ts: None,
        cpp: None,
        side: Some(Side::Guest),
    },
    WitTarget {
        wit: CORE,
        rs: Some("rust/client/runtime/src/abi.rs"),
        ts: Some("ts/client/src/abi"),
        cpp: None,
        side: Some(Side::Host),
    },
];

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    for t in &GPU_TARGETS {
        out.extend(wit_outputs(&root(), t, &[])?);
    }
    for t in &CORE_TARGETS {
        out.extend(wit_outputs(&root(), t, &GPU_TARGETS)?);
    }
    Ok(out)
}
