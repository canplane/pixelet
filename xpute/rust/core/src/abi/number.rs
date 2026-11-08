// xpute-core/abi/number.rs

//! A number a packet carries is read as its field's type. What the runtime
//! sends is checked here, so that no code past the door (`ipc::door`) meets a
//! NaN, an infinity, or an integer forced into range by a cast.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

/// The error for a number that no member of an enum is given, where the
/// enum has no member for such a number to read as.
#[track_caller]
pub fn unknown() -> MarshalError {
    MarshalError::new(Errno::EINVAL)
}

#[track_caller]
pub fn finite(n: f64) -> Result<f64, MarshalError> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(unknown())
    }
}

/// Reads a whole number that the type holds. A number outside the type's range
/// is refused, never cast into it.
#[track_caller]
pub fn integer<T: TryFrom<i64>>(n: f64) -> Result<T, MarshalError> {
    let whole = finite(n)? == n.trunc() && n.abs() <= (1u64 << f64::MANTISSA_DIGITS) as f64;
    if whole {
        T::try_from(n as i64).map_err(|_| unknown())
    } else {
        Err(unknown())
    }
}
