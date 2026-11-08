// pxl-core/sys/io/runtime.rs

//! The door through which the core asks its runtime for its files and sends
//! its report. Each ask is a frame of xpute's own commands, which the runtime
//! takes after the turn returns. Memory that a frame names belongs to the
//! runtime until the runtime's head has passed the frame. Besides this
//! module, only the files (sys/io/file.rs) post through the door, and
//! `FILE_FRAMES` bounds what they post.

use core::panic::Location;

use xpute_core::global::Global;
use xpute_core::ipc::door::Door;
use xpute_core::ipc::fs::FD_TURN_FRAMES;
use xpute_core::machine;
use xpute_kit::status::errno::Errno;

use crate::sys::io::param::{MAX_CREDITS, WRITES_A_TURN};

/// The most frames the files may post in one turn. Each credit may have its
/// file answered and a new one opened on it, the storage file may be read and
/// written, and every write the turn allows may be posted. Each of these is
/// counted at the most frames one descriptor posts in a turn
/// (`FD_TURN_FRAMES`).
pub const FILE_FRAMES: u32 = FD_TURN_FRAMES * (2 * MAX_CREDITS + 2 + WRITES_A_TURN);

static TURN: Global<u64> = Global::new();

/// The door the machine opened on the front the runtime laid out. A native
/// test has no runtime, and so no door.
pub fn door() -> Option<&'static Door> {
    machine::booted().then(machine::door)
}

/// Begins a turn. The door lets go of the reads the runtime has taken.
pub fn rise() {
    *TURN.get(|| 0) += 1;
    if let Some(d) = door() {
        d.rise();
    }
}

pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

/// Reports a broken invariant. The report is posted at once as the turn's
/// only frame and the turn's other frames are dropped, because the core
/// stops here (xpute_core::stop).
pub fn abort(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    if let Some(d) = door() {
        d.abort(errnum, at, a, b);
    }
}
