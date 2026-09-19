// xpute-core/status/error.rs

use core::fmt;

use crate::status::errno::Errno;

#[derive(Default)]
pub struct XputeErrorOptions {
    pub cause: Option<Box<dyn std::error::Error>>,
}

/// Error policy:
/// - Internal invariant violation (our bug / impossible state) => FATAL (crash process).
/// - External fault (peer/network/remote/env/input) => NON-FATAL (catch, isolate, degrade, drop).
///
/// Notes:
/// - Errno describes the error code, not fatality.
/// - Fatal vs non-fatal is determined by error class (and catch boundary policy).
/// - Origin/context should be attached at catch/log boundary, not encoded in message prefixes.
///
/// An error is returned: `Result<_, XputeError>`, or a subclass. The
/// subclasses are newtypes over this, each dereferencing to it, so a
/// handler that takes the base takes any of them.
#[derive(Debug)]
pub struct XputeError {
    pub errno: Errno,
    pub message: String,
    pub cause: Option<Box<dyn std::error::Error>>,
}

impl XputeError {
    pub fn new(errno: Errno, message: Option<&str>, opts: Option<XputeErrorOptions>) -> XputeError {
        let msg = match message {
            Some(m) if !m.is_empty() => m.to_string(),
            _ => format!("errno: {}", errno as i32),
        };
        XputeError {
            errno,
            message: msg,
            cause: opts.and_then(|o| o.cause),
        }
    }
}

impl fmt::Display for XputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for XputeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_deref()
    }
}

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
            pub fn new(errno: Errno, message: Option<&str>, opts: Option<XputeErrorOptions>) -> $name {
                $name($base::new(errno, message, opts))
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
                write!(f, "{}: {}", stringify!($name), self.0.message)
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.0.source()
            }
        }

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
