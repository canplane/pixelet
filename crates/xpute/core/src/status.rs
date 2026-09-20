// xpute-core/status.rs

//! How the kit says what happened: the number a call answers with, the
//! failure that carries one, and where a line a person reads goes.
//!
//! `Errno::OK` is a success, which is why this is not `error`. What a call
//! answers with is a status whether or not anything went wrong — a kernel
//! function returns 0 or a negative errno, and both are this.

#[path = "status/errno.spec.rs"]
pub mod errno;
pub mod error;
pub mod log;
