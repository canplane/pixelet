// xpute-kit/status/error.rs

use core::fmt;
use core::panic::Location;

use crate::status::errno::Errno;

/// An errno and the place it was made, no message. The class decides
/// fatality: an invariant violation is fatal, an external fault is not.
#[derive(Debug)]
pub struct XputeError {
    pub errno: Errno,
    pub at: &'static Location<'static>,
}

impl XputeError {
    #[track_caller]
    pub fn new(errno: Errno) -> XputeError {
        XputeError { errno, at: Location::caller() }
    }
}

impl fmt::Display for XputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {}", self.errno, self.at)
    }
}

impl std::error::Error for XputeError {}

#[derive(Debug)]
pub struct InvariantError(pub XputeError);

#[derive(Debug)]
pub struct FaultError(pub XputeError);

#[derive(Debug)]
pub struct MarshalError(pub FaultError);

#[derive(Debug)]
pub struct NetworkFaultError(pub FaultError);

macro_rules! subclass {
    ($name:ident extends $base:ident) => {
        impl $name {
            #[track_caller]
            pub fn new(errno: Errno) -> $name {
                $name($base::new(errno))
            }
        }

        impl core::ops::Deref for $name {
            type Target = $base;
            fn deref(&self) -> &$base {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}: {}", stringify!($name), self.0)
            }
        }

        impl std::error::Error for $name {}

        impl From<$name> for XputeError {
            fn from(e: $name) -> XputeError {
                e.0.into()
            }
        }
    };
}

subclass!(InvariantError extends XputeError);
subclass!(FaultError extends XputeError);
subclass!(MarshalError extends FaultError);
subclass!(NetworkFaultError extends FaultError);
