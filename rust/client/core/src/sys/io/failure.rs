// pxl-core/sys/io/failure.rs

//! How a read's attempt ended, and why a failed one failed.

use std::borrow::Cow;
use std::collections::TryReserveError;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;

pub(crate) use crate::spec::demand::OutcomeName as Outcome;
use crate::sys::io::room::Refusal;

/// How an attempt ended, as one value, so an attempt that ended well cannot
/// carry a failure and a failed one always carries its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Exit {
    Ok,
    Aborted,
    Failed(Failure),
    /// The heap or the body slots refused the body room. Neither the object
    /// nor the server failed, and the want may start again once what refused
    /// it gives some room back (`Refusal::released`).
    Refused(Refusal),
}

/// Why an attempt failed: the error it failed with, its errno, the number
/// the statistics count by, the place it was made, its file and its line,
/// and its message, the core's own or one that came across the link as its
/// maker's side made it; and a detail the core adds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Failure {
    pub errnum: Errno,
    pub detail: u16,
    pub file: Cow<'static, str>,
    pub line: u32,
    pub msg: Option<Box<str>>,
}

impl Exit {
    pub(crate) fn outcome(&self) -> Outcome {
        match self {
            Exit::Ok => Outcome::Ok,
            Exit::Aborted => Outcome::Aborted,
            Exit::Failed(_) | Exit::Refused(_) => Outcome::Err,
        }
    }
}

/// How an attempt that failed with `f` ends: one the host canceled, as
/// a reload of its program leaves an open, is aborted, so its want starts
/// again; any other failed.
pub(crate) fn exit_of(f: Failure) -> Exit {
    if is_aborted(f.errnum) {
        Exit::Aborted
    } else {
        Exit::Failed(f)
    }
}

/// Whether an attempt that failed with `errnum` is aborted (`exit_of`).
pub(crate) fn is_aborted(errnum: Errno) -> bool {
    errnum == Errno::ECANCELED
}

impl From<Result<(), Exit>> for Exit {
    fn from(result: Result<(), Exit>) -> Exit {
        result.err().unwrap_or(Exit::Ok)
    }
}

/// The heap refused a tile's lanes the room, so the want waits on the heap
/// (sys/io/room.rs).
impl From<TryReserveError> for Exit {
    fn from(_: TryReserveError) -> Exit {
        Exit::Refused(Refusal::now())
    }
}

impl From<Failure> for Exit {
    fn from(fail: Failure) -> Exit {
        Exit::Failed(fail)
    }
}

impl From<XputeError> for Exit {
    #[track_caller]
    fn from(err: XputeError) -> Exit {
        Exit::Failed(Failure::from(err))
    }
}

impl From<Errno> for Failure {
    #[track_caller]
    fn from(errnum: Errno) -> Failure {
        Failure::from(XputeError::new(errnum))
    }
}

impl From<XputeError> for Failure {
    fn from(err: XputeError) -> Failure {
        Failure {
            errnum: err.errnum,
            detail: 0,
            file: err.file,
            line: err.line,
            msg: err.msg,
        }
    }
}
