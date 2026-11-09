// xpute-conformance/lib.rs

//! This is the Rust conformance core: a library that a runtime opens and runs
//! the traces under spec/conformance/ against.

#[path = "core.spec.rs"]
pub mod spec;

pub mod core;
