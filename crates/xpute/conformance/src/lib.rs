// xpute-conformance/lib.rs

//! The Rust conformance guest, built as a library a host opens: the few
//! commands spec/xpute/conformance/guest.json numbers, over xpute-runtime,
//! computing nothing. What holds it to the contract (ARCHITECTURE.xpute.md)
//! are the traces under spec/xpute/conformance/, which one host runs against every guest —
//! this one, and cpp/xpute/conformance/ (packages/xpute/conformance/).

#[path = "guest.spec.rs"]
pub mod spec;

pub mod guest;
