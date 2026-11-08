// pxl-core/sys/io/runtime.rs

//! What the core asks of its runtime through the door: its files and its
//! report, frames of xpute's own commands the runtime takes after the turn
//! returns, what each names the runtime's until its head has passed it. Only
//! the files post through it beside this module, and `FILE_FRAMES` counts
//! what they post.

use core::panic::Location;

use xpute_core::global::Global;
use xpute_core::ipc::door::Door;
use xpute_core::ipc::fs::FD_TURN_FRAMES;
use xpute_core::machine;
use xpute_kit::status::errno::Errno;

use crate::sys::io::param::{MAX_CREDITS, WRITES_A_TURN};

/// What the files may post in one turn: each credit's file answered and a
/// new one opened on it, the storage file read and written, and the writes a
/// turn allows, each descriptor's frames at their most.
pub const FILE_FRAMES: u32 = FD_TURN_FRAMES * (2 * MAX_CREDITS + 2 + WRITES_A_TURN);

static TURN: Global<u64> = Global::new();

/// The door the machine opened on the front the runtime laid; none in a
/// native test, which has no runtime.
pub fn door() -> Option<&'static Door> {
    machine::booted().then(machine::door)
}

/// A turn begins: counted, and the reads the runtime has taken let go.
pub fn rise() {
    *TURN.get(|| 0) += 1;
    if let Some(d) = door() {
        d.rise();
    }
}

pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

/// A broken invariant: posted at once as the turn's only frame, the rest
/// dropped, as the core stops here (xpute_core::stop).
pub fn abort(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    if let Some(d) = door() {
        d.abort(errnum, at, a, b);
    }
}
