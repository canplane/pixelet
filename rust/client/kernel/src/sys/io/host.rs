// pxl-kernel/sys/io/host.rs
//
// What the kernel asks of the host, as xpute system calls recorded during a
// turn and run by the host after it returns, so they are done before the
// next turn rises. Only the clock is called directly, since a turn measures
// its budget mid-turn.

use core::panic::Location;

use xpute_guest::global::Global;
use xpute_guest::ipc::stream::WORD;
use xpute_guest::ipc::sys::{fd_turn_bytes, Sys};
use xpute_guest::mem;
use xpute_kit::status::errno::Errno;

use crate::sys::io::file::{Kept, PATH_BYTES};
use crate::sys::io::host_spec::stream;
use crate::sys::io::param::MAX_CREDITS;

static SYS: Global<Option<Sys>> = Global::new();

/// Only the file table records through it beside this module, and `RESERVE`
/// counts both.
pub(crate) fn sys() -> Option<&'static mut Sys> {
    SYS.get(|| None).as_mut()
}

/// # Safety
/// `at` must address `stream::BYTES` nothing else writes, 4-aligned, for as
/// long as the module runs.
pub unsafe fn set_stream(at: u32) {
    // SAFETY: the caller's range.
    SYS.set(Some(unsafe { Sys::new(mem::core(), at as usize, stream::BYTES as usize / WORD) }));
}

pub fn rise() {
    *TURN.get(|| 0) += 1;
    if let Some(h) = sys() {
        h.rise();
    }
}

/// The most one file records in a turn: its descriptor's.
const FILE_TURN_BYTES: usize = fd_turn_bytes(PATH_BYTES);

/// What the calls that cannot wait may record in one turn, all of them files:
/// each credit's file answered and a new one opened on it, and each kept file
/// read and written. Requests that can wait take the stream only past it.
pub const RESERVE: usize = FILE_TURN_BYTES * (2 * MAX_CREDITS as usize + 2 * Kept::ALL.len());

const _: () = assert!(RESERVE < stream::BYTES as usize, "the calls that cannot wait would not fit the host stream");

/// A request that can wait a turn: None where there is no host, false where
/// the turn's stream has no room for it past `RESERVE`.
pub fn offer(op: u32, args: &[u32], bytes: &[u8]) -> Option<bool> {
    Some(sys()?.try_call(op, args, bytes, RESERVE))
}

static TURN: Global<u64> = Global::new();

/// What a record asked for on turn `n` is done on any turn after it.
pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

pub fn fall() {
    if let Some(h) = sys() {
        h.fall();
    }
}

/// A broken invariant: published at once as the turn's only record, dropping
/// the rest, since the kernel stops here (sys/stop.rs).
pub fn abort(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    if let Some(h) = sys() {
        h.abort(errnum, at, a, b);
    }
}

/// A host's answer as one value: a count, or the errno it refused with. A code
/// no errno names is the host breaking the protocol.
pub fn answered(res: i32) -> Result<u32, Errno> {
    match u32::try_from(res) {
        Ok(n) => Ok(n),
        Err(_) => Err(Errno::of(res).unwrap_or(Errno::EPROTO)),
    }
}
