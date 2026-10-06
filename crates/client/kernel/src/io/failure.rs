// pxl-kernel/io/failure.rs

//! What a task's run came to, and why it failed.

use core::fmt;
use core::panic::Location;

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

pub use crate::sched::profiler::spec::OutcomeName as Outcome;

/// Why a task failed, as numbers the summary can count by. A kernel-made
/// failure carries its source location: EBADMSG alone does not say which
/// rule a body broke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Failure {
    pub errno: i32,
    pub detail: u16,
    pub at: Option<&'static Location<'static>>,
}

impl From<Errno> for Failure {
    #[track_caller]
    fn from(errno: Errno) -> Failure {
        Failure {
            errno: errno as i32,
            detail: 0,
            at: Some(Location::caller()),
        }
    }
}

impl From<MarshalError> for Failure {
    fn from(err: MarshalError) -> Failure {
        Failure {
            errno: err.errno as i32,
            detail: 0,
            at: Some(err.at),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.errno, self.detail) {
            (0, _) => Ok(()),
            (errno, 0) => write!(f, "errno {errno}"),
            (errno, detail) => write!(f, "errno {errno} {detail}"),
        }
    }
}
