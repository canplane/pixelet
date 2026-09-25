// pxl-kernel/host.rs
//
// What the kernel asks of the host that is not a command it answers: its
// clock, the browser's key-value storage where the object store keeps its
// blob, the wall clock, the text measure. The host installs the calls before
// the first turn (pxl-wasm's `install_host`); a native test runs on the
// defaults, which keep nothing and hold time at zero.

use crate::global::Global;

/// The host's clock, in milliseconds. The substrate holds it
/// (xpute clock.rs); it is named here because it arrives with the rest.
pub use xpute_runtime::clock::{now, set_clock};

pub struct Storage {
    pub get: fn(key: &str) -> Option<String>,
    pub set: fn(key: &str, value: &str),
}

fn get_nothing(_: &str) -> Option<String> {
    None
}

fn set_nothing(_: &str, _: &str) {}

static STORAGE: Global<Storage> = Global::new();

/// Installs `localStorage`.
pub fn set_storage(storage: Storage) {
    STORAGE.set(storage);
}

fn storage() -> &'static Storage {
    STORAGE.get(|| Storage { get: get_nothing, set: set_nothing })
}

/// `localStorage.getItem(key)`.
pub fn storage_get(key: &str) -> Option<String> {
    (storage().get)(key)
}

/// `localStorage.setItem(key, value)`.
pub fn storage_set(key: &str, value: &str) {
    (storage().set)(key, value)
}

static BODY_READ: Global<fn(body: u32, dst: &mut [u8])> = Global::new();

fn read_nothing(_: u32, _: &mut [u8]) {}

/// Installs the host's read of a body it holds for the kernel (a response, a
/// model file): copies it into `dst` and lets it go.
pub fn set_body_read(read: fn(body: u32, dst: &mut [u8])) {
    BODY_READ.set(read);
}

/// Copies body `body` into `dst`, which is its length, and the host lets it
/// go; an empty `dst` only lets it go.
pub fn body_read(body: u32, dst: &mut [u8]) {
    (BODY_READ.get(|| read_nothing))(body, dst)
}

static HANDHELD: Global<bool> = Global::new();

/// Installs whether the device is a phone or a tablet (shared/lib/browser/handheld).
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

static MEASURE: Global<fn(font: &str, text: &str) -> f64> = Global::new();

fn measure_nothing(_: &str, text: &str) -> f64 {
    text.chars().count() as f64 * 7.0
}

/// Installs a canvas context's `measureText`: the width `text` takes in `font`.
pub fn set_measure_text(measure: fn(font: &str, text: &str) -> f64) {
    MEASURE.set(measure);
}

/// `ctx.font = font; ctx.measureText(text).width`.
pub fn measure_text(font: &str, text: &str) -> f64 {
    (MEASURE.get(|| measure_nothing))(font, text)
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

static LOCAL_TIME: Global<fn() -> LocalTime> = Global::new();

fn no_time() -> LocalTime {
    LocalTime::default()
}

/// Installs the host's local wall clock.
pub fn set_local_time(clock: fn() -> LocalTime) {
    LOCAL_TIME.set(clock);
}

pub fn local_time() -> LocalTime {
    (LOCAL_TIME.get(|| no_time))()
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
}

static HEAP: Global<fn() -> HeapStat> = Global::new();

fn no_heap() -> HeapStat {
    HeapStat::default()
}

/// Installs the module's allocator's reading.
pub fn set_heap_stat(stat: fn() -> HeapStat) {
    HEAP.set(stat);
}

/// The module's heap as it stands; every word 0 where there is none.
pub fn heap_stat() -> HeapStat {
    (HEAP.get(|| no_heap))()
}
