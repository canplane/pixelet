// pxl-core/sys/io/runtime.rs
//
// What the core asks of its runtime: its files and its report, frames of
// xpute's own commands that the runtime takes after the turn returns, what each
// names the runtime's until its head has passed it.

use core::panic::Location;

use xpute_core::global::Global;
use xpute_core::ipc::door::Door;
use xpute_core::ipc::fs::FD_TURN_FRAMES;
use xpute_core::machine;
use xpute_kit::status::errno::Errno;

use crate::sys::io::file::StorageFile;
use crate::sys::io::param::{MAX_CREDITS, WRITES_A_TURN};

/// The door, once the machine has opened the front the runtime laid; none
/// in a native test, which has no runtime. Only the file table posts through
/// it beside this module, and `FILE_FRAMES` counts what the files post.
pub(crate) fn door() -> Option<&'static Door> {
    machine::booted().then(machine::door)
}

pub fn rise() {
    *TURN.get(|| 0) += 1;
    if let Some(d) = door() {
        d.rise();
    }
}

/// What the files may post in one turn: each credit's file answered and a
/// new one opened on it, each storage file read and written, and the writes
/// by number a turn allows, each descriptor's frames at their most.
pub const FILE_FRAMES: u32 =
    FD_TURN_FRAMES * (2 * MAX_CREDITS + 2 * StorageFile::ALL.len() as u32 + WRITES_A_TURN);

static TURN: Global<u64> = Global::new();

/// The turn's number, counted by `rise`.
pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

/// A broken invariant: posted at once as the turn's only frame, dropping the
/// rest, since the core stops here (xpute_core::stop).
pub fn abort(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    if let Some(d) = door() {
        d.abort(errnum, at, a, b);
    }
}
