// pxl-kernel/io/host.rs
//
// What the kernel asks of the host, as xpute system calls recorded during a
// turn and run by the host after it returns, so they are done before the
// next turn rises. Only the clock is called directly, since a turn measures
// its budget mid-turn.

use core::panic::Location;
pub use xpute_guest::mem::heap::HeapStat;

use xpute_guest::ipc::stream::{record_bytes, WORD};
use xpute_guest::ipc::sys::{descriptor_turn_bytes, Sys};
use xpute_guest::mem::base::ptr_at;
use xpute_kit::status::errno::Errno;

use crate::abi::host::{op, stream};
use crate::io::file::{Kept, PATH_BYTES};
use crate::param::MAX_CREDITS;
use xpute_guest::global::Global;

static SYS: Global<Option<Sys>> = Global::new();

/// Only the file table records through it beside this module, and `RESERVE`
/// counts both.
pub(crate) fn sys() -> Option<&'static mut Sys> {
    SYS.get(|| None).as_mut()
}

/// # Safety
/// `at` must address `stream::BYTES` nothing else writes, 4-aligned, for as
/// long as the module runs.
pub unsafe fn set_host_stream(at: u32) {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize), stream::BYTES as usize / WORD) };
    SYS.set(Some(Sys::new(words)));
}

pub fn host_rise() {
    *TURN.get(|| 0) += 1;
    if let Some(h) = sys() {
        h.rise();
    }
}

const DECODE_ARGS: usize = 2;

/// The one program call that cannot wait, counted in `RESERVE`.
pub fn decode_image(fd: u32, image: u32) {
    if let Some(h) = sys() {
        let args: [u32; DECODE_ARGS] = [fd, image];
        h.call(op::IMAGE_DECODE, &args, &[]);
    }
}

/// The most one file records in a turn: its descriptor's, and an image's decode.
const FILE_TURN_BYTES: usize = descriptor_turn_bytes(PATH_BYTES) + record_bytes(DECODE_ARGS, Some(0));

/// What the calls that cannot wait may record in one turn, all of them files:
/// each credit's file answered and a new one opened on it, and each kept file
/// read and written. Requests that can wait take the stream only past it.
pub const RESERVE: usize = FILE_TURN_BYTES * (2 * MAX_CREDITS as usize + 2 * Kept::ALL.len());

const _: () = assert!(RESERVE < stream::BYTES as usize, "the calls that cannot wait would not fit the host stream");

/// A request that can wait a turn: None where there is no host, false where
/// the turn's stream has no room for it past `RESERVE`.
pub fn host_offer(op: u32, args: &[u32], bytes: &[u8]) -> Option<bool> {
    Some(sys()?.try_call(op, args, bytes, RESERVE))
}

static TURN: Global<u64> = Global::new();

/// What a record asked for on turn `n` is done on any turn after it.
pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

pub fn host_fall() {
    if let Some(h) = sys() {
        h.fall();
    }
}

/// A broken invariant: published at once as the turn's only record, dropping
/// the rest, since the kernel stops here. Also told to the bracket
/// (`last_words`) where the process dies before a host could run it.
/// A host's answer as one value: a count, or the errno it refused with. A code
/// no errno names is the host breaking the protocol.
pub fn answered(res: i32) -> Result<u32, Errno> {
    match u32::try_from(res) {
        Ok(n) => Ok(n),
        Err(_) => Err(Errno::of(res).unwrap_or(Errno::EPROTO)),
    }
}

pub fn host_bug(errno: Errno, at: &Location<'_>, a: u64, b: u64) {
    STOPPED.set(true);
    last_words(Stop::Bug { errno, at, a, b });
    if let Some(h) = sys() {
        h.abort(errno, at, a, b);
    }
}

static STOPPED: Global<bool> = Global::new();

/// Past its report a bug panics where it cannot trap, and that panic is its
/// way out, not a second fault.
pub fn stopped() -> bool {
    *STOPPED.get(|| false)
}

static HANDHELD: Global<bool> = Global::new();

pub fn set_handheld(handheld: bool) {
    HANDHELD.set(handheld);
}

pub fn handheld() -> bool {
    *HANDHELD.get(|| false)
}

static DEVELOPMENT: Global<bool> = Global::new();

/// Development builds offer the settings schema.json marks `dev`.
pub fn set_development(development: bool) {
    DEVELOPMENT.set(development);
}

pub fn development() -> bool {
    *DEVELOPMENT.get(|| false)
}

static WALL_MS: Global<f64> = Global::new();

pub fn set_wall_now(unix_ms: f64) {
    WALL_MS.set(unix_ms);
}

pub fn wall_now() -> f64 {
    *WALL_MS.get(|| 0.0)
}

static HEAP: Global<fn() -> HeapStat> = Global::new();

fn no_heap() -> HeapStat {
    HeapStat::default()
}

pub fn set_heap_stat(stat: fn() -> HeapStat) {
    HEAP.set(stat);
}

/// Numbers only: formatting is the bracket's, so a module that reports
/// nothing links none of it.
pub enum Stop<'a> {
    Bug { errno: Errno, at: &'a Location<'a>, a: u64, b: u64 },
    Panic(&'a std::panic::PanicHookInfo<'a>),
}

pub type LastWords = fn(Stop<'_>);

static LAST_WORDS: Global<Option<LastWords>> = Global::new();

pub fn set_last_words(say: Option<LastWords>) {
    LAST_WORDS.set(say);
}

pub fn last_words(stop: Stop<'_>) {
    if let Some(say) = *LAST_WORDS.get(|| None) {
        say(stop);
    }
}

pub fn heap_stat() -> HeapStat {
    (HEAP.get(|| no_heap))()
}
