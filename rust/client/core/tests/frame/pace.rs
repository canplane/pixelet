//! The core's clock, the display's, from its boot: each frame's turn reads the time it was
//! granted at, a refresh after the last, and the clock stands through the turn and between
//! turns, while the harness waits on the device and rings the UI's calls. What the core
//! steps by its own clock (a character, the location indicator, a request's age) then steps at
//! the display's pace whatever the machine takes, loaded or not, and a wake it asks at its own
//! step reads the same; what it budgets within a turn finds the turn's deadline unspent and runs
//! to its end.

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

/// The core's clock made the display's, before its first turn (xpute-core `entry::run`
/// installs its boot's clock only where none is set).
pub fn install(core: Core) {
    core.run_on(|| clock::set(now))
        .expect("the core has stopped");
}

/// A frame's turn is granted, a refresh after the last.
pub fn frame() {
    *AT.lock().unwrap_or_else(|e| e.into_inner()) += REFRESH_MS;
}
