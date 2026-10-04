// pxl-kernel/host.rs
//
// What the kernel asks of the host, as xpute system calls recorded during a
// turn and run by the host after it returns, so they are done before the
// next turn rises. Only the clock is called directly, since a turn measures
// its budget mid-turn.

use core::panic::Location;

use xpute_core::status::errno::Errno;
use xpute_runtime::ipc::sys::Sys;
use xpute_runtime::mem::base::ptr_at;

use crate::abi::host::stream;
use crate::global::Global;

pub use xpute_runtime::clock::{now, set_clock};

static SYS: Global<Option<Sys>> = Global::new();

pub fn sys() -> Option<&'static mut Sys> {
    SYS.get(|| None).as_mut()
}

/// # Safety
/// `at` must address `stream::BYTES` nothing else writes, 4-aligned, for as
/// long as the module runs.
pub unsafe fn set_host_stream(at: u32) {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize), (stream::BYTES / 4) as usize) };
    SYS.set(Some(Sys::new(words)));
}

pub fn host_rise() {
    *TURN.get(|| 0) += 1;
    if let Some(h) = host() {
        h.rise();
    }
}

/// False where there is no host to run it.
pub fn host_record(op: u32, args: &[u32], bytes: &[u8]) -> bool {
    let Some(h) = host() else { return false };
    h.call(op, args, bytes);
    true
}

static TURN: Global<u64> = Global::new();

/// What a record asked for on turn `n` is done on any turn after it.
pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

pub fn host_fall() {
    if let Some(h) = host() {
        h.fall();
    }
}

/// A broken invariant: published at once as the turn's only record, dropping
/// the rest, since the kernel stops here. Also told to the bracket
/// (`last_words`) where the process dies before a host could run it.
pub fn host_bug(errno: Errno, at: &Location<'_>, a: u64, b: u64) {
    STOPPED.set(true);
    last_words(Stop::Bug { errno, at, a, b });
    if let Some(h) = host() {
        h.abort(errno, at, a, b);
    }
}

static STOPPED: Global<bool> = Global::new();

/// Past its report a bug panics where it cannot trap, and that panic is its
/// way out, not a second fault.
pub fn stopped() -> bool {
    *STOPPED.get(|| false)
}

/// What the platform keeps under `storage`'s key (host.json): a file of the
/// host's, numbered by `storage`.
pub fn storage_set(storage: u32, value: String) {
    let Some(h) = host() else { return };
    h.write(storage, value.into_bytes());
}

/// Asks the host to copy body `body` into `dst`, which is its length, and let
/// it go; an empty `dst` only lets it go. The bytes are there from the next
/// turn on, and `dst` must be kept until then.
pub fn body_read(body: u32, dst: &mut [u8]) {
    let Some(h) = host() else { return };
    h.read(body, dst);
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalTime {
    pub unix_ms: f64,
    pub hours: f64,
    pub minutes: f64,
    pub seconds: f64,
    pub day_start_ms: f64,
}

static LOCAL_TIME: Global<LocalTime> = Global::new();

pub fn set_local_time(t: LocalTime) {
    LOCAL_TIME.set(t);
}

pub fn local_time() -> LocalTime {
    *LOCAL_TIME.get(LocalTime::default)
}

pub fn wall_now() -> f64 {
    local_time().unix_ms
}

/// One reading of the allocator, so compared fields are from one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapStat {
    pub range: usize,
    /// A high-water mark, not a holding.
    pub used: usize,
    pub live: usize,
    pub run: usize,
    pub slot: usize,
    /// Past the tree's current root the range is untouched rather than listed,
    /// so this is not the free space.
    pub largest_free: usize,
    /// Moves exactly when a request refused before might now be served.
    pub releases: u32,
    pub refusals: u32,
    pub requests: u32,
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
