// xpute-runtime/lib.rs

//! This is the runtime's side in Rust; `@xpute/runtime` plays the same role in
//! TypeScript. `mem` reads and writes a core's memory by offset on any
//! platform. `native` is the runtime for a core linked into the runtime's own
//! process; it exists only on unix, because it maps the core's memory through
//! libc. `jni` exports that native core to the Kotlin of an Android app.
//! `abort_report` is the core's stop report as the runtime logs it.
//!
//! A program that links its core in writes one xpute-core `core!` over its
//! manifest, and outside WebAssembly `core!` expands to `native_core!`. A
//! program that lays out its own memory instead declares the allocator and
//! writes one `native_abi!`, as this example does:
//!
#![cfg_attr(unix, doc = concat!("```\n", include_str!("../examples/native.rs"), "```"))]

pub mod abi;
pub mod mem;

mod report;
pub use report::abort_report;

#[cfg(unix)]
pub mod native;

#[cfg(target_os = "android")]
pub mod jni;
