// xpute-host/lib.rs

//! The host's end in Rust, the role `@xpute/host` plays in TypeScript, each
//! holding what its platform needs. `mem` reads a guest's memory by offset on
//! any platform; `native` hosts a guest linked into the host's own process
//! (unix's, mapping its memory through libc), and `jni` exports it to an
//! Android app's Kotlin. A program that links its guest in writes one
//! xpute-guest `guest!` over its machine, which expands to `native_guest!`
//! off WebAssembly; beneath it, one that lays out its own memory declares
//! the allocator and writes one `native_abi!`:
//!
#![cfg_attr(unix, doc = concat!("```\n", include_str!("../examples/native.rs"), "```"))]

pub mod mem;

#[cfg(unix)]
pub mod native;

#[cfg(target_os = "android")]
pub mod jni;
