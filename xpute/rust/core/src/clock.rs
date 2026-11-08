// xpute-core/clock.rs

//! The runtime's clock, in milliseconds, counted as the UI's
//! `performance.now()` counts them, so that what the core times and what the UI
//! times lie on one line. Until a runtime installs one, the time stays at zero,
//! which native tests run with.

use core::cell::Cell;

thread_local! {
    static CLOCK: Cell<Option<fn() -> f64>> = const { Cell::new(None) };
}

pub fn set(clock: fn() -> f64) {
    CLOCK.with(|c| c.set(Some(clock)));
}

pub fn is_set() -> bool {
    CLOCK.with(Cell::get).is_some()
}

pub fn now() -> f64 {
    CLOCK.with(Cell::get).map_or(0.0, |f| f())
}
