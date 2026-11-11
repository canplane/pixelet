// xpute-runtime/report.rs

use xpute_core::abi::sys::SysAbort;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

/// The core's stop report, SYS_ABORT's packet, as the runtime logs it: the
/// errno of the broken invariant by its name, where in the source it broke,
/// and the two numbers the core gave, each joined from its two words. It is
/// the line the TypeScript runtime logs (runtime.ts `abort_report`).
pub fn abort_report(pkt: &[u8]) -> Result<String, MarshalError> {
    let (errnum, line, column, a_low, a_high, b_low, b_high, file) = SysAbort::parts(pkt)?;
    let word = |low: u32, high: u32| u64::from(high) << 32 | u64::from(low);
    let name = Errno::of(errnum).map_or_else(|| errnum.to_string(), |e| format!("{e:?}"));
    let (a, b) = (word(a_low, a_high), word(b_low, b_high));
    Ok(format!(
        "[xpute] stopped: {name} at {file}:{line}:{column} ({a}, {b})"
    ))
}
