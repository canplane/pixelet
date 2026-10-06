// xpute-runtime/clock.rs

//! The host's clock in milliseconds; zero until installed, as in native tests.

use core::cell::Cell;

thread_local! {
    static CLOCK: Cell<Option<fn() -> f64>> = const { Cell::new(None) };
}

pub fn set_clock(clock: fn() -> f64) {
    CLOCK.with(|c| c.set(Some(clock)));
}

pub fn now() -> f64 {
    CLOCK.with(|c| c.get()).map_or(0.0, |f| f())
}
