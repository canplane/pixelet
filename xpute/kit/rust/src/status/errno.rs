// xpute-kit/status/errno.rs

//! The errno a guest and its host agree on (`kit/spec/errno.json5`): POSIX's
//! and Linux's numbers, negated.

#[path = "errno.spec.rs"]
pub mod spec;
pub use spec::{Errno, Kind};
