// pxl-core/sys/io/failure.rs

//! How a run ended, and why one failed.

use core::panic::Location;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub use crate::sys::io::demand_spec::OutcomeName as Outcome;
use crate::sys::io::file::Opened;
use crate::sys::io::room::Refusal;

/// One value, so a run that ended well carries no failure and a failed one
/// always does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Ok,
    Aborted,
    Failed(Failure),
    /// Its body was refused room, by the heap or the body slots: no failure
    /// of the object's or the server's, and over once what refused it gives
    /// some back (`Refusal::over`).
    Refused(Refusal),
}

/// Why a run failed, as numbers the statistics count by; one the core made
/// carries its source location.
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

impl From<Result<(), Failure>> for Exit {
    fn from(result: Result<(), Failure>) -> Exit {
        match result {
            Ok(()) => Exit::Ok,
            Err(fail) => Exit::Failed(fail),
        }
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

/// An open that came to no bytes.
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
