// xpute-core/status/error.rs

use core::fmt;

use crate::status::errno::Errno;

/// Error policy:
/// - Internal invariant violation (our bug / impossible state) => FATAL (crash process).
/// - External fault (peer/network/remote/env/input) => NON-FATAL (catch, isolate, degrade, drop).
///
/// Notes:
/// - Errno describes the error code, not fatality.
/// - Fatal vs non-fatal is determined by error class (and catch boundary policy).
/// - Origin/context should be attached at catch/log boundary, not encoded in the error.
///
/// An error is its errno and its class, and nothing else: what it means to a
/// reader is `strerror` of the number, where the reader is. A sentence carried
/// with it would say what the errno and the place it was raised at already
/// say, in a language the module then ships.
///
/// An error is returned: `Result<_, XputeError>`, or a subclass. The
/// subclasses are newtypes over this, each dereferencing to it, so a
/// handler that takes the base takes any of them.
#[derive(Debug)]
pub struct XputeError {
    pub errno: Errno,
}

impl XputeError {
    pub fn new(errno: Errno) -> XputeError {
        XputeError { errno }
    }
}

impl fmt::Display for XputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.errno)
    }
}

impl std::error::Error for XputeError {}

/// Internal invariant violation / impossible state / our bug (fatal).
#[derive(Debug)]
pub struct InvariantError(pub XputeError);

/// External fault / untrusted input / peer-network-remote failure (normally non-fatal).
#[derive(Debug)]
pub struct FaultError(pub XputeError);

/// Wire/payload decode failure at marshal boundary (not an application-level error, normally non-fatal).
#[derive(Debug)]
pub struct MarshalError(pub FaultError);

/// Network/transport/protocol I/O fault (normally non-fatal).
#[derive(Debug)]
pub struct NetworkFaultError(pub FaultError);

/// What `extends` gives a subclass: the base's constructor, its fields
/// through `Deref`, and its place in the error chain.
macro_rules! subclass {
    ($name:ident extends $base:ident) => {
        impl $name {
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
