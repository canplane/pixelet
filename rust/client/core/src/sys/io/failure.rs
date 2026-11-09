// pxl-core/sys/io/failure.rs

//! How a read's attempt ended, and why a failed one failed.

use core::panic::Location;
use std::collections::TryReserveError;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub use crate::sys::io::demand_spec::OutcomeName as Outcome;
use crate::sys::io::file::Opened;
use crate::sys::io::room::Refusal;

/// How an attempt ended, as one value, so an attempt that ended well cannot
/// carry a failure and a failed one always carries its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Ok,
    Aborted,
    Failed(Failure),
    /// The heap or the body slots refused the body room. Neither the object
    /// nor the server failed, and the want may start again once what refused
    /// it gives some room back (`Refusal::over`).
    Refused(Refusal),
}

/// Why an attempt failed, in the numbers the statistics count by. A failure
/// the core raised itself carries the source location that raised it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    pub errnum: Errno,
    pub detail: u16,
    pub at: Option<&'static Location<'static>>,
}

impl Exit {
    pub fn outcome(&self) -> Outcome {
        match self {
            Exit::Ok => Outcome::ok,
            Exit::Aborted => Outcome::aborted,
            Exit::Failed(_) | Exit::Refused(_) => Outcome::err,
        }
    }
}

impl From<Result<(), Exit>> for Exit {
    fn from(result: Result<(), Exit>) -> Exit {
        result.err().unwrap_or(Exit::Ok)
    }
}

/// The heap refused a tile's columns the room, so the want waits on the heap
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

impl From<MarshalError> for Exit {
    #[track_caller]
    fn from(err: MarshalError) -> Exit {
        Exit::Failed(Failure::from(err))
    }
}

impl From<Errno> for Failure {
    #[track_caller]
    fn from(errnum: Errno) -> Failure {
        Failure {
            errnum,
            detail: 0,
            at: Some(Location::caller()),
        }
    }
}

/// The failure of an open that brought no bytes. An open that brought bytes
/// is no failure, and taking it for one is a bug.
impl From<Opened> for Failure {
    #[track_caller]
    fn from(opened: Opened) -> Failure {
        match opened {
            Opened::Failed { errnum, detail } => Failure {
                errnum,
                detail: detail as u16,
                at: None,
            },
            Opened::Absent => Failure::from(Errno::ENOENT),
            Opened::Bytes(_) => Failure::from(Errno::ENOTRECOVERABLE),
        }
    }
}

impl From<MarshalError> for Failure {
    fn from(err: MarshalError) -> Failure {
        Failure {
            errnum: err.errnum,
            detail: 0,
            at: Some(err.at),
        }
    }
}
