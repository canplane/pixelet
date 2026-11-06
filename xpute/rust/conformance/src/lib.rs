// xpute-conformance/lib.rs

//! The Rust conformance core, a library a runtime opens and runs the traces
//! under spec/conformance/ against.

#[path = "core.spec.rs"]
pub mod spec;

pub mod core;
