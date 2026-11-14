// xpute-kit/status/error.rs

use core::fmt;
use core::panic::Location;

use crate::status::errno::{Errno, Kind};

/// This is an errno and the place in the source where the error was made, with
/// no message. It is returned, never fatal: a broken invariant is a bug
/// (status/bug.rs). What happened is the errno's `kind`.
#[derive(Debug)]
pub struct XputeError {
    pub errnum: Errno,
    pub at: &'static Location<'static>,
}

impl XputeError {
    #[track_caller]
    pub fn new(errnum: Errno) -> XputeError {
        XputeError {
            errnum,
            at: Location::caller(),
        }
    }

    pub const fn kind(&self) -> Kind {
        self.errnum.kind()
    }
}

impl fmt::Display for XputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {}", self.errnum, self.at)
    }
}

impl std::error::Error for XputeError {}
