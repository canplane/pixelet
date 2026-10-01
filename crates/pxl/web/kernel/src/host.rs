// pxl-kernel/host.rs
//
// What the kernel asks of the host that is not a command it answers, made as
// xpute's system calls (xpute-runtime ipc/sys.rs): its files (io/file.rs),
// and the kernel's own calls — a text laid out or drawn (text.rs), an image
// decoded (io/load.rs) — numbered in abi/host.json. The host runs
// them once the turn has returned, so what one asks is done by the time the
// next turn rises — and what it names is kept that long. Only the clock is
// still called: a turn measures its own budget mid-turn, when the host cannot
// run. A native test has no stream, and storage keeps nothing there.

use core::panic::Location;

use xpute_core::status::errno::Errno;
use xpute_runtime::ipc::sys::Sys;
use xpute_runtime::mem::base::ptr_at;

use crate::abi::host::stream;
use crate::global::Global;

/// The host's clock, in milliseconds. The substrate holds it
/// (xpute clock.rs); it is named here because it arrives with the rest.
pub use xpute_runtime::clock::{now, set_clock};

static SYS: Global<Option<Sys>> = Global::new();

/// The system calls, where there is a host to run them.
pub fn sys() -> Option<&'static mut Sys> {
    SYS.get(|| None).as_mut()
}

/// Installs the system calls' stream at `at`, `stream::BYTES` of the
/// kernel's memory nothing else writes.
///
/// # Safety
/// `at` must address that range, 4-aligned, for as long as the module runs.
pub unsafe fn set_host_stream(at: u32) {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize), (stream::BYTES / 4) as usize) };
    SYS.set(Some(Sys::new(words)));
}

/// The turn's rise: what the host did after the last turn is in, so what was
/// held for it goes.
pub fn host_rise() {
    *TURN.get(|| 0) += 1;
    if let Some(h) = sys() {
        h.rise();
    }
}

/// The kernel's own call `op` (abi/host.json) with `args` and `bytes`, for
/// the host to run once the turn has returned; false where there is no host
/// to run it.
pub fn host_record(op: u32, args: &[u32], bytes: &[u8]) -> bool {
    let Some(h) = sys() else { return false };
    h.call(op, args, bytes);
    true
}

static TURN: Global<u64> = Global::new();

/// Which turn this is. What a record asked for on turn `n` is done on any
/// turn after it.
pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

/// The turn's fall: its records, for the host to run once it returns.
pub fn host_fall() {
    if let Some(h) = sys() {
        h.fall();
    }
}

/// A broken invariant (xpute-core status/bug.rs): its errno, its place and
/// its two numbers, as the one record of the turn, published at once for the
/// host to find when the trap reaches it. What was recorded before it in the
/// turn is dropped: the kernel stops here, and none of it is to be done.
///
/// Where the kernel stops with its process, before any host could run the
/// record, the bracket says the report as well (`last_words`).
pub fn host_bug(errno: Errno, at: &Location<'_>, a: u64, b: u64) {
    STOPPED.set(true);
    last_words(Stop::Bug { errno, at, a, b });
    if let Some(h) = sys() {
        h.abort(errno, at, a, b);
    }
}

static STOPPED: Global<bool> = Global::new();

/// Whether the kernel has reported what stopped it: past its report a bug
/// panics where it cannot trap (xpute-core status/bug.rs), and that panic is
/// its way out rather than a second fault.
pub fn stopped() -> bool {
    *STOPPED.get(|| false)
}

static HANDHELD: Global<bool> = Global::new();

/// Installs whether the device is a phone or a tablet (the platform's `HANDHELD`, apps/web/src/platform/web/core.ts).
pub fn set_handheld(handheld: bool) {
    HANDHELD.set(handheld);
}

/// `HANDHELD`.
pub fn handheld() -> bool {
    *HANDHELD.get(|| false)
}

static DEVELOPMENT: Global<bool> = Global::new();

/// Installs whether the build is a development one (the shell's
/// `__IS_DEV__`), which offers the settings schema.json marks `dev`.
pub fn set_development(development: bool) {
    DEVELOPMENT.set(development);
}

pub fn development() -> bool {
    *DEVELOPMENT.get(|| false)
}

/// The local wall clock: `Date.now()`, and the local hour and the local day's
/// start as a `Date` reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalTime {
    pub unix_ms: f64,
    pub hours: f64,
    pub minutes: f64,
    pub seconds: f64,
    /// The local midnight that began today, in unix ms.
    pub day_start_ms: f64,
}

static LOCAL_TIME: Global<LocalTime> = Global::new();

/// The wall clock as the frame's command brought it.
pub fn set_local_time(t: LocalTime) {
    LOCAL_TIME.set(t);
}

pub fn local_time() -> LocalTime {
    *LOCAL_TIME.get(LocalTime::default)
}

/// `Date.now()`.
pub fn wall_now() -> f64 {
    local_time().unix_ms
}

/// What the module's heap is, at one moment (xpute-core alloc/slab.rs).
///
/// One reading rather than a gauge apiece: they are one allocator's state and
/// a caller comparing two of them is comparing one moment, not two. `live` is
/// what the page level has handed out with a carved run counted whole, so it
/// is what the heap has committed and not what anything holds — `slot` is
/// that, and `run` less `slot` is what the carving costs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapStat {
    /// The bytes of the range the heap was given.
    pub range: usize,
    /// The bytes it has ever reached into: a high-water mark, not a holding.
    pub used: usize,
    /// Bytes the page level has handed out and not taken back.
    pub live: usize,
    /// Of `live`, what was borrowed to be carved into slots.
    pub run: usize,
    /// Of `run`, what is out in slots, each at its class's size.
    pub slot: usize,
    /// The largest block on a free list. Past the tree's current root the
    /// range is untouched rather than listed, so this is not the free space.
    pub largest_free: usize,
    /// Blocks taken back. It moves when, and only when, a request refused
    /// before might now be served, which is what makes it a signal and not a
    /// gauge.
    pub releases: u32,
    /// Requests answered with nothing: the one way this memory is full, since
    /// the fixed ranges cannot be.
    pub refusals: u32,
    /// Requests the page level has served: a turn that makes none has asked
    /// the heap for nothing.
    pub requests: u32,
}

static HEAP: Global<fn() -> HeapStat> = Global::new();

fn no_heap() -> HeapStat {
    HeapStat::default()
}

/// Installs the module's allocator's reading.
pub fn set_heap_stat(stat: fn() -> HeapStat) {
    HEAP.set(stat);
}

/// What stopped the kernel, as a bracket is told it: the numbers and the
/// place, never a sentence. Making one is the bracket's, so that only a
/// bracket that says it links the formatting — a name for every errno, a
/// panic's message — and the module, which says nothing, carries none of it.
pub enum Stop<'a> {
    /// A broken invariant (xpute-core status/bug.rs).
    Bug { errno: Errno, at: &'a Location<'a>, a: u64, b: u64 },
    /// A panic no invariant named: an index past a slice, a wgpu validation
    /// error, whose text is not in any number.
    Panic(&'a std::panic::PanicHookInfo<'a>),
}

/// Where a bracket says what stopped the kernel (door.rs `Bracket`).
pub type LastWords = fn(Stop<'_>);

static LAST_WORDS: Global<Option<LastWords>> = Global::new();

pub fn set_last_words(say: Option<LastWords>) {
    LAST_WORDS.set(say);
}

/// Says what stopped the kernel, where the bracket has somewhere to.
pub fn last_words(stop: Stop<'_>) {
    if let Some(say) = *LAST_WORDS.get(|| None) {
        say(stop);
    }
}

/// The module's heap as it stands; every word 0 where there is none.
pub fn heap_stat() -> HeapStat {
    (HEAP.get(|| no_heap))()
}
