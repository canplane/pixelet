//! The guest's clock, the display's, from its boot: each frame's turn reads the time it was
//! granted at, a refresh after the last, and the clock stands through the turn and between
//! turns, while the harness waits on the device and rings the shell's calls. What the kernel
//! steps by its own clock (an actor, the location indicator, a request's age) then steps at
//! the display's pace whatever the machine takes, loaded or not, and a wake it asks at its own
//! step reads the same; what it budgets within a turn finds the turn's quota unspent and runs
//! to its end.

use std::sync::Mutex;

use xpute_guest::clock;
use xpute_host::native::Guest;

use crate::REFRESH_MS;

/// The time the current frame's turn was granted at.
static AT: Mutex<f64> = Mutex::new(0.0);

/// The display's clock in milliseconds.
fn now() -> f64 {
    *AT.lock().unwrap_or_else(|e| e.into_inner())
}

/// The guest's clock made the display's, before its first turn (xpute-guest `entry::run`
/// installs its boot's clock only where none is set).
pub fn install(guest: Guest) {
    guest.run(|| clock::set(now));
}

/// A frame's turn is granted, a refresh after the last.
pub fn frame() {
    *AT.lock().unwrap_or_else(|e| e.into_inner()) += REFRESH_MS;
}
