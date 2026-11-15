// pxl-tool/spec/cmd.rs

//! The core's link's bindings from spec/client/core/wit/, written by
//! xpute-link-gen as any program's are, over xpute's own packages, which it
//! carries itself: the guest's Rust in pxl-core, and a host's, the Rust that
//! pxl-runtime serves of pixelet's own interfaces (rust/client/runtime/src/text.rs)
//! on xpute-native-host's link and the TypeScript the UI takes.

use xpute_link_gen::wit::{wit_outputs, Side, WitTarget};

use super::Output;
use crate::root;

const WIT: &str = "spec/client/core/wit";

pub fn outputs() -> Result<Vec<Output>, String> {
    let guest = WitTarget {
        wit: WIT,
        rs: Some("rust/client/core/src/abi.rs"),
        ts: None,
        cpp: None,
        side: Some(Side::Guest),
    };
    let host = WitTarget {
        wit: WIT,
        rs: Some("rust/client/runtime/src/abi.rs"),
        ts: Some("ts/client/src/abi"),
        cpp: None,
        side: Some(Side::Host),
    };
    let mut out = wit_outputs(&root(), &guest)?;
    out.extend(wit_outputs(&root(), &host)?);
    Ok(out)
}
