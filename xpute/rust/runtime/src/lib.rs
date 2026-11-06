// xpute-runtime/lib.rs

//! The runtime's end in Rust, the role `@xpute/runtime` plays in TypeScript, each
//! holding what its platform needs. `mem` reads a core's memory by offset on
//! any platform; `native` runtimes a core linked into the runtime's own process
//! (unix's, mapping its memory through libc), and `jni` exports it to an
//! Android app's Kotlin. A program that links its core in writes one
//! xpute-core `core!` over its machine, which expands to `native_core!`
//! off WebAssembly; beneath it, one that lays out its own memory declares
//! the allocator and writes one `native_abi!`:
//!
#![cfg_attr(unix, doc = concat!("```\n", include_str!("../examples/native.rs"), "```"))]

pub mod mem;

#[cfg(unix)]
pub mod native;

#[cfg(target_os = "android")]
pub mod jni;
