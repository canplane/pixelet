//! The generic kit, in Rust: `@xpute/core`'s counterpart, and held to the
//! same rule — no pixelet domain knowledge, so that it stays portable to
//! other projects. Collections and lanes, the codecs and XTP, the errno
//! table both languages are written from, and the libm that agrees with V8
//! to the bit.
//!
//! What runs *over* it — the rings, the turn, the memory a module is given —
//! is `xpute-runtime`, which depends on this and not the other way round.
//! Taking this out of the project takes nothing of the project with it.

pub mod cmd;
pub mod ds;
pub mod errno;
pub mod error;
pub mod golden;
pub mod io;
pub mod kit;
pub mod log;
