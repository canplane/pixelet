// pxl-kernel/sys/io/failure.rs

//! What a task's run came to, and why it failed.

use core::panic::Location;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

use crate::sys::io::room::Refused;

pub use crate::sys::io::demand_spec::OutcomeName as Outcome;

/// Why a task failed, as numbers the summary can count by. A kernel-made
/// failure carries its source location: EBADMSG alone does not say which
/// rule a body broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Failure {
    pub errnum: Errno,
    pub detail: u16,
    pub at: Option<&'static Location<'static>>,
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

impl From<MarshalError> for Failure {
    fn from(err: MarshalError) -> Failure {
        Failure {
            errnum: err.errnum,
            detail: 0,
            at: Some(err.at),
        }
    }
}

/// How a run ended: one value, so a run that ended well carries no failure
/// and a failed one always does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    Ok,
    Aborted,
    Failed(Failure),
    /// Its body was refused room, by the heap or the body slots: no failure
    /// of the object's or the server's, and over once what refused it gives
    /// some back (`Refused::over`).
    Refused(Refused),
}

impl Ended {
    pub fn outcome(&self) -> Outcome {
        match self {
            Ended::Ok => Outcome::ok,
            Ended::Aborted => Outcome::aborted,
            Ended::Failed(_) | Ended::Refused(_) => Outcome::error,
        }
    }
}

impl From<Result<(), Failure>> for Ended {
    fn from(result: Result<(), Failure>) -> Ended {
        match result {
            Ok(()) => Ended::Ok,
            Err(fail) => Ended::Failed(fail),
        }
    }
}
