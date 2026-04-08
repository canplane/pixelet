// @xpute/spec/sys/error.rs

use crate::sys::errno::Errno;

/// Error policy:
/// - Internal invariant violation (our bug / impossible state) => FATAL (crash process).
/// - External fault (peer/network/remote/env/input) => NON-FATAL (catch, isolate, degrade, drop).
///
/// Notes:
/// - Errno describes the error code, not fatality.
/// - Fatal vs non-fatal is determined by error class (and catch boundary policy).
/// - Origin/context should be attached at catch/log boundary, not encoded in message prefixes.
#[derive(Debug, Clone)]
pub struct XputeError {
    pub errno: Errno,
    pub message: String,
}

impl XputeError {
    pub fn new(errno: Errno, message: impl Into<String>) -> Self {
        let message = message.into();
        let message = if message.is_empty() {
            format!("errno: {}", errno)
        } else {
            message
        };
        Self { errno, message }
    }
}

impl std::fmt::Display for XputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for XputeError {}

/// Internal invariant violation / impossible state / our bug (fatal).
#[derive(Debug, Clone)]
pub struct InvariantError(pub XputeError);

impl InvariantError {
    pub fn new(errno: Errno, message: impl Into<String>) -> Self {
        Self(XputeError::new(errno, message))
    }
    pub fn errno(&self) -> Errno { self.0.errno }
    pub fn message(&self) -> &str { &self.0.message }
}

impl std::fmt::Display for InvariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for InvariantError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<InvariantError> for XputeError {
    fn from(e: InvariantError) -> Self { e.0 }
}

/// External fault / untrusted input / peer-network-remote failure (normally non-fatal).
#[derive(Debug, Clone)]
pub struct FaultError(pub XputeError);

impl FaultError {
    pub fn new(errno: Errno, message: impl Into<String>) -> Self {
        Self(XputeError::new(errno, message))
    }
    pub fn errno(&self) -> Errno { self.0.errno }
    pub fn message(&self) -> &str { &self.0.message }
}

impl std::fmt::Display for FaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for FaultError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<FaultError> for XputeError {
    fn from(e: FaultError) -> Self { e.0 }
}

/// Wire/payload decode failure at marshal boundary (not an application-level error, normally non-fatal).
#[derive(Debug, Clone)]
pub struct MarshalError(pub XputeError);

impl MarshalError {
    pub fn new(errno: Errno, message: impl Into<String>) -> Self {
        Self(XputeError::new(errno, message))
    }
    pub fn errno(&self) -> Errno { self.0.errno }
    pub fn message(&self) -> &str { &self.0.message }
}

impl std::fmt::Display for MarshalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MarshalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<MarshalError> for FaultError {
    fn from(e: MarshalError) -> Self { Self(e.0) }
}

impl From<MarshalError> for XputeError {
    fn from(e: MarshalError) -> Self { e.0 }
}

/// Network/transport/protocol I/O fault (normally non-fatal).
#[derive(Debug, Clone)]
pub struct NetworkFaultError(pub XputeError);

impl NetworkFaultError {
    pub fn new(errno: Errno, message: impl Into<String>) -> Self {
        Self(XputeError::new(errno, message))
    }
    pub fn errno(&self) -> Errno { self.0.errno }
    pub fn message(&self) -> &str { &self.0.message }
}

impl std::fmt::Display for NetworkFaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for NetworkFaultError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<NetworkFaultError> for FaultError {
    fn from(e: NetworkFaultError) -> Self { Self(e.0) }
}

impl From<NetworkFaultError> for XputeError {
    fn from(e: NetworkFaultError) -> Self { e.0 }
}
