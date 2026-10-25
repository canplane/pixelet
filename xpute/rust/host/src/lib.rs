// xpute-host/lib.rs

//! The host's end in Rust. A program that links its guest in declares the
//! allocator, boots the guest once with its numbers, and calls into it
//! through its own C ABI (examples/native.rs):
//!
#![cfg_attr(unix, doc = concat!("```\n", include_str!("../examples/native.rs"), "```"))]
//!
//! It is the host's end as `@xpute/host` (ts/host) is in TypeScript: one
//! role, each holding what its platform needs. Rust's holds one form of host
//! today, `native`, for a guest linked into the host's own native process.
//! Ringing the doorbell, answering the system calls and pacing the quantum
//! are TypeScript's: xpute was written for a program that drives its guest
//! from TypeScript, wherever the guest runs.
//!
//! `mem` is every host's: the guest's memory read and written by offset,
//! whatever holds it. `native` is unix's, since it maps the core memory from
//! the system through libc; elsewhere the crate is `mem` alone, and so are
//! its tests.

pub mod mem;

#[cfg(unix)]
pub mod native;
