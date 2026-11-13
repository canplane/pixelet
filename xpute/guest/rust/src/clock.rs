// xpute-guest/clock.rs

//! The host's clocks (wit/clocks/monotonic-clock.wit,
//! wit/clocks/system-clock.wit), called in an entry. They count nanoseconds, as
//! wasi:clocks does; the program reads them in ms, converted here alone.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::clocks::{monotonic_clock, system_clock};

const NS_PER_MS: f64 = 1e6;

/// The time in ms, which never runs backward; only differences mean anything.
pub fn now() -> f64 {
    monotonic_clock::now().or_bug(Errno::EPROTO) as f64 / NS_PER_MS
}

/// `ms` as a duration the monotonic clock waits for, rounded up.
pub fn duration(ms: f64) -> u64 {
    (ms.max(0.0) * NS_PER_MS).ceil() as u64
}

/// The system clock, in Unix ms.
pub fn system() -> f64 {
    let t = system_clock::now().or_bug(Errno::EPROTO);
    t.seconds as f64 * 1e3 + f64::from(t.nanoseconds) / NS_PER_MS
}
