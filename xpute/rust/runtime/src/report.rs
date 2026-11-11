// xpute-runtime/report.rs

use xpute_kit::status::errno::Errno;

use crate::abi::sys::SysAbort;

/// The core's stop report, SYS_ABORT's packet as read, as the runtime logs
/// it: the errno of the broken invariant by its name, where in the source it
/// broke, and the two numbers the core gave, each joined from its two words.
/// It is the line the TypeScript runtime logs (runtime.ts `abort_report`).
pub fn abort_report(e: &SysAbort<'_>) -> String {
    let word = |low: u32, high: u32| u64::from(high) << 32 | u64::from(low);
    let name = Errno::of(e.errnum).map_or_else(|| e.errnum.to_string(), |e| format!("{e:?}"));
    let (a, b) = (word(e.a_low, e.a_high), word(e.b_low, e.b_high));
    format!(
        "[xpute] stopped: {name} at {}:{}:{} ({a}, {b})",
        e.file, e.line, e.column
    )
}
