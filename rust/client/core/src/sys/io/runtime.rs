// pxl-core/sys/io/runtime.rs

//! The door through which the core asks its runtime for its files and sends
//! its report. Each ask is a frame of xpute's own commands. Memory that a
//! file's read fills belongs to the runtime until the close of its descriptor
//! is completed (xpute_core io/door.rs R1). Besides this module, only the
//! files (sys/io/file.rs) send through the door.

use core::panic::Location;

use xpute_core::io::door::{self, Door};
use xpute_kit::status::errno::Errno;

/// The door opened on the front the runtime laid out (xpute-core io/door.rs `boot`).
pub fn door() -> &'static Door {
    door::door()
}

/// Reports a broken invariant. The report is sent at once, after what the
/// turn has sent, because the core stops here (xpute_core::stop).
pub fn abort(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    if door::booted() {
        door().abort(errnum, at, a, b);
    }
}
