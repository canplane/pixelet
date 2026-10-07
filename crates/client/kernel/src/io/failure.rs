// pxl-kernel/io/failure.rs

//! What a task's run came to, and why it failed.

use core::fmt;
use core::panic::Location;

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

pub use crate::io::spec::OutcomeName as Outcome;

/// Why a task failed, as numbers the summary can count by. A kernel-made
/// failure carries its source location: EBADMSG alone does not say which
/// rule a body broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Failure {
    pub errno: Errno,
    pub detail: u16,
    pub at: Option<&'static Location<'static>>,
}

impl From<Errno> for Failure {
    #[track_caller]
    fn from(errno: Errno) -> Failure {
        Failure {
            errno,
            detail: 0,
            at: Some(Location::caller()),
        }
    }
}

impl From<MarshalError> for Failure {
    fn from(err: MarshalError) -> Failure {
        Failure {
            errno: err.errno,
            detail: 0,
            at: Some(err.at),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.detail {
            0 => write!(f, "errno {}", self.errno as i32),
            detail => write!(f, "errno {} {detail}", self.errno as i32),
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
}

impl Ended {
    pub fn outcome(&self) -> Outcome {
        match self {
            Ended::Ok => Outcome::ok,
            Ended::Aborted => Outcome::aborted,
            Ended::Failed(_) => Outcome::error,
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
