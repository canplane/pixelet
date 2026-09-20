// xpute-core/lib.rs

//! The generic kit, in Rust: `@xpute/core`'s counterpart, and held to the
//! same rule — no domain knowledge of the program over it, so that it
//! stays portable to another one. Collections and lanes, the codecs and XTP, the errno
//! table both languages are written from, and the libm every end computes
//! with, so a double is the same bits wherever it is made.
//!
//! What runs *over* it — the rings, the turn, the memory a module is given —
//! is `xpute-runtime`, which depends on this and not the other way round.
//! Taking this out of the project takes nothing of the project with it.

pub mod abi;
pub mod alloc;
pub mod codec;
pub mod collection;
pub mod golden;
pub mod math;
pub mod status;
pub mod wire;
