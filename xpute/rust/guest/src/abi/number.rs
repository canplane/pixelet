// xpute-guest/abi/number.rs

//! A number a packet carries, read as its field's type. What the host sends is
//! checked here, so nothing past the door meets a NaN, an infinity or an
//! integer cast into range.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

/// A number an enum does not give out, where the enum has no member it reads as.
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

/// A whole number the type holds, never one cast into it.
#[track_caller]
pub fn integer<T: TryFrom<i64>>(n: f64) -> Result<T, MarshalError> {
    let whole = finite(n)? == n.trunc() && n.abs() <= (1u64 << f64::MANTISSA_DIGITS) as f64;
    if whole {
        T::try_from(n as i64).map_err(|_| unknown())
    } else {
        Err(unknown())
    }
}
