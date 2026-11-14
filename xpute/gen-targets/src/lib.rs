// xpute-gen-targets/lib.rs

//! xpute's generation targets, which the tools that generate code read rather
//! than know: its bindings on each side and their root modules, and its data
//! specs, the errno table among them. Paths are from xpute's root.

use xpute_gen::spec::data::DataSpec;
use xpute_link_gen::wit::{Side, WitTarget};

/// The bindings of the link's own types, which every side's bindings read
/// first.
pub const LINK_TARGET: WitTarget<'static> = WitTarget {
    wit: "link/wit",
    rs: Some("link/rust/src/abi.rs"),
    ts: Some("link/ts/src/abi"),
    side: None,
};

/// Everything else of xpute's packages follows from their names, each
/// `xpute:<name>` in `wit/<name>`, and from each side's crate: the guest's
/// bindings, with the TypeScript beside the link's, and the native host's,
/// each in its side's root module, `src/abi.rs`.
macro_rules! packages {
    (guest: $g:literal, native: $n:literal; $($p:ident),*) => {
        /// The root modules of each side's bindings: the guest's and the
        /// native host's.
        pub const ABI_ROOTS: [&str; 2] = [concat!($g, "/src/abi.rs"), concat!($n, "/src/abi.rs")];

        /// xpute's bindings on the guest's side.
        pub const GUEST_TARGETS: &[WitTarget<'static>] = &[$(WitTarget {
            wit: concat!("wit/", stringify!($p)),
            rs: Some(concat!($g, "/src/abi/", stringify!($p), ".rs")),
            ts: Some(concat!("link/ts/src/abi/", stringify!($p))),
            side: Some(Side::Guest($g)),
        }),*];

        /// xpute's bindings on a native device's host's side.
        pub const NATIVE_HOST_TARGETS: &[WitTarget<'static>] = &[$(WitTarget {
            wit: concat!("wit/", stringify!($p)),
            rs: Some(concat!($n, "/src/abi/", stringify!($p), ".rs")),
            ts: None,
            side: Some(Side::Host($n)),
        }),*];
    };
}

packages!(guest: "guest/rust", native: "native/host"; clocks, fs, life, stop);

/// xpute's data specs: the link's frame, and the errno table.
pub const DATA_SPECS: [DataSpec; 2] = [
    DataSpec {
        json: "link/spec/frame.json",
        ts: Some("link/ts/src/frame.spec.ts"),
        rs: Some("link/rust/src/frame.spec.rs"),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "kit/spec/errno.json",
        ts: Some("kit/ts/src/status/errno.spec.ts"),
        rs: Some("kit/rust/src/status/errno.spec.rs"),
        cpp: Some(("cpp/xpute/kit/status/errno.spec.hpp", "xpute")),
    },
];
