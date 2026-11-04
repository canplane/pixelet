// pxl-kernel/sys/io/host.rs
//
// What the kernel asks of its host and the client beside it: its files and
// its report, frames for the host's port that the host takes after the turn
// returns, so they are done before the next turn rises; and the client's
// text, called on the shell's port and answered before the call returns.

use core::panic::Location;

use xpute_guest::global::Global;
use xpute_guest::ipc::door::Door;
use xpute_guest::ipc::sys::FD_TURN_FRAMES;
use xpute_kit::status::errno::Errno;

use crate::sys::io::file::StorageFile;
use crate::sys::io::param::{MAX_CREDITS, WRITES_A_TURN};
use crate::sys::io::text::TextCall;

/// What the kernel's top hands this module once the rings are laid.
pub struct Host {
    pub door: &'static Door,
    /// A text call made of the client, answered before it returns.
    pub text: fn(&TextCall<'_>) -> Result<u32, Errno>,
}

static HOST: Global<Option<Host>> = Global::new();

/// The rings are laid, so the files and the text reach a host from now on.
pub fn set_host(host: Host) {
    HOST.set(Some(host));
}

/// The door, where there is a host: only the file table posts through it
/// beside this module, and `FILE_FRAMES` counts what the files post.
pub(crate) fn door() -> Option<&'static Door> {
    HOST.get(|| None).as_ref().map(|h| h.door)
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

/// A text call, answered before it returns: None where there is no host, else
/// its value or its errno, ENOSPC where no slot is free or it does not fit one.
pub fn call(text: &TextCall<'_>) -> Option<Result<u32, Errno>> {
    let h = HOST.get(|| None).as_ref()?;
    Some((h.text)(text))
}

static TURN: Global<u64> = Global::new();

/// What a frame asked for on turn `n` is done on any turn after it.
pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

/// A broken invariant: posted at once as the turn's only frame, dropping the
/// rest, since the kernel stops here (xpute_guest::stop).
pub fn abort(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    if let Some(d) = door() {
        d.abort(errnum, at, a, b);
    }
}
