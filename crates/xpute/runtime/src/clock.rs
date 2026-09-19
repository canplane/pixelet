// xpute-runtime/clock.rs

//! The host's clock, in milliseconds: what `performance.now()` is to a
//! script. The host installs it before the first turn; until then time
//! stands at zero, which is what a native test that never installs one runs
//! on.

use core::cell::Cell;

thread_local! {
    static CLOCK: Cell<Option<fn() -> f64>> = const { Cell::new(None) };
}

/// Installs the host's clock.
pub fn set_clock(clock: fn() -> f64) {
    CLOCK.with(|c| c.set(Some(clock)));
}

/// The host's time, in milliseconds.
pub fn now() -> f64 {
    CLOCK.with(|c| c.get()).map_or(0.0, |f| f())
}
