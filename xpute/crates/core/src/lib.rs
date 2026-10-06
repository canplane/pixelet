// xpute-core/lib.rs

//! The generic kit, `@xpute/core`'s counterpart: no domain knowledge.
//! `xpute-runtime` depends on it, never the reverse.

pub mod abi;
pub mod alloc;
pub mod codec;
pub mod collection;
pub mod golden;
pub mod math;
pub mod status;
pub mod wire;
