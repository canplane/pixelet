// xpute-conformance/lib.rs

//! What holds a guest to the contract (ARCHITECTURE.xpute.md): traces of
//! what a host does and what it must then see (golden/), and the guest they
//! drive, which computes nothing and implements the few commands
//! spec/xpute/conformance/guest.json numbers.
//!
//! A trace is written from the contract's rules and not recorded from any
//! guest, and it holds only what a host can observe — replies, signals,
//! stream records, what a turn asks for next — never a guest's own sizes or
//! how it keeps its output in bounds. This crate's guest is the Rust one;
//! another language's guest implements the same commands and is run through
//! the same traces.

#[path = "guest.spec.rs"]
pub mod spec;

pub mod guest;

#[cfg(test)]
#[path = "trace.test.rs"]
mod trace;
