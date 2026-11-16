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
    ms(monotonic_clock::now().or_bug(Errno::EPROTO))
}

/// A mark of the monotonic clock in ms, on `now`'s line, or a duration
/// between two in ms: a time the host answers, as a display's frame start,
/// or a span of one, as the gap since the frame before.
pub fn ms(mark: monotonic_clock::Mark) -> f64 {
    mark as f64 / NS_PER_MS
}

/// A time in ms on `now`'s line as a mark of the monotonic clock, rounded
/// down, and none before the clock's start (0): a time the program asks the
/// host for, as a display's frame start.
pub fn mark(ms: f64) -> monotonic_clock::Mark {
    (ms.max(0.0) * NS_PER_MS).floor() as monotonic_clock::Mark
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
