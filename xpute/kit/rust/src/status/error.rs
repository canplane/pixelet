// xpute-kit/status/error.rs

use core::fmt;
use core::panic::Location;

use crate::status::errno::{Errno, Kind};

/// This is an errno, the place in the source where the error was made, and
/// what a person reads of it beyond the errno, where its maker says more. It
/// is returned, never fatal: a broken invariant is a bug (`status/bug.rs`).
/// What happened is the errno's `kind`; across the link it is its errno
/// alone.
#[derive(Debug)]
pub struct XputeError {
    pub errnum: Errno,
    pub at: &'static Location<'static>,
    /// What a person reads beyond the errno, made once and never grown, so
    /// it holds no capacity.
    pub msg: Option<Box<str>>,
}

impl XputeError {
    #[track_caller]
    pub fn new(errnum: Errno) -> XputeError {
        XputeError {
            errnum,
            at: Location::caller(),
            msg: None,
        }
    }

    /// The error `errnum`, with what a person reads of it.
    #[track_caller]
    pub fn with_msg(errnum: Errno, msg: impl Into<Box<str>>) -> XputeError {
        XputeError {
            errnum,
            at: Location::caller(),
            msg: Some(msg.into()),
        }
    }

    pub const fn kind(&self) -> Kind {
        self.errnum.kind()
    }
}

impl fmt::Display for XputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {}", self.errnum, self.at)?;
        match &self.msg {
            Some(m) => write!(f, ": {m}"),
            None => Ok(()),
        }
    }
}

impl std::error::Error for XputeError {}

/// What a person reads of the errno `errnum`: its name and its text, or its
/// number where no errno is it, as @xpute/kit `strerror` writes it.
pub fn strerror(errnum: i32) -> String {
    Errno::of(errnum).map_or_else(
        || format!("errno {errnum}"),
        |e| format!("{}: {}", e.name(), e.text()),
    )
}
