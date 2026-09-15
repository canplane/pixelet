//! The generic runtime kit, in Rust: @xpute/core's counterpart, and held to
//! the same rule — no pixelet domain knowledge, so that it stays portable
//! to other projects. What lives here is what the core needs from a
//! runtime and a browser does not give it: memory carved into pools and
//! a heap over a fixed range, the rings and the change log between two
//! sides of a linear memory, and the libm that agrees with V8 to the bit. The tree mirrors packages/xpute/core/src.

pub mod ds;
pub mod io;
pub mod ipc;
pub mod kit;
pub mod sched;
pub mod sys;
#[cfg(any(test, feature = "vector"))]
pub mod vector;
