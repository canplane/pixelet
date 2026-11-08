//! Replaces the core's clock with the display's from boot. Each frame's turn reads the time
//! it was granted at, one refresh after the last, and the clock stands still through the
//! turn and between turns, while the harness waits on the device and rings the UI's calls.
//! What the core steps by its own clock (a character, the location indicator, a request's
//! age) therefore steps at the display's pace however long the machine takes, loaded or
//! not, and a wake the core asks for at its own step reads the same. Work the core budgets
//! within a turn always finds the turn's deadline unspent and runs to its end.

use std::sync::Mutex;

use xpute_core::clock;
use xpute_runtime::native::Core;

use crate::REFRESH_MS;

/// The time the current frame's turn was granted at.
static AT: Mutex<f64> = Mutex::new(0.0);

/// The display's clock in milliseconds.
fn now() -> f64 {
    *AT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sets the core's clock to the display's before its first turn. xpute-core `entry::run`
/// installs the boot's clock only when none is set, so this one stays.
pub fn install(core: Core) {
    core.run_on(|| clock::set(now))
        .expect("the core has stopped");
}

/// Advances the clock by one refresh, for the next frame's turn.
pub fn frame() {
    *AT.lock().unwrap_or_else(|e| e.into_inner()) += REFRESH_MS;
}
