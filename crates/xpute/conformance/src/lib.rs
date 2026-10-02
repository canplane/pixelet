// xpute-conformance/lib.rs

//! The Rust conformance guest, a library a host opens and runs the traces
//! under spec/xpute/conformance/ against.

#[path = "guest.spec.rs"]
pub mod spec;

pub mod guest;
