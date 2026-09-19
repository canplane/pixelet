// xpute-core/status/log.rs
// (no pair: JavaScript's `console` is the runtime's)

//! Where a `console.warn` goes. The module has no console; a host installs
//! one (`set_warn`), and until it does a warning is dropped, as it would be
//! with the console closed.

use core::cell::Cell;

thread_local! {
    static WARN: Cell<Option<fn(&str)>> = const { Cell::new(None) };
}

/// Installs the host's `console.warn`.
pub fn set_warn(f: fn(&str)) {
    WARN.with(|w| w.set(Some(f)));
}

/// `console.warn(message)`.
pub fn warn(message: &str) {
    if let Some(f) = WARN.with(|w| w.get()) {
        f(message);
    }
}
