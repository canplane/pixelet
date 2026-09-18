// pxl-kernel/sys/host.rs
//
// What the kernel asks of the host that is not a command it answers: the
// browser's key-value storage, where the object store keeps its blob. The
// host installs the calls before the first turn; a native test runs on the
// defaults, which keep nothing.

use crate::sys::global::Global;

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

static HEAP_USED: Global<fn() -> usize> = Global::new();

fn no_heap() -> usize {
    0
}

/// Installs the module's allocator's reading: the bytes its heap reaches to.
pub fn set_heap_used(used: fn() -> usize) {
    HEAP_USED.set(used);
}

/// The bytes the module's heap reaches to; 0 where there is none.
pub fn heap_used() -> usize {
    (HEAP_USED.get(|| no_heap))()
}

static HEAP_RELEASES: Global<fn() -> u32> = Global::new();

fn no_releases() -> u32 {
    0
}

/// Installs the reading a build refused a block waits on: the count of blocks
/// the heap has taken back. It moves when, and only when, a request refused
/// before might now be served, which is what makes it a signal and not a
/// gauge.
pub fn set_heap_releases(releases: fn() -> u32) {
    HEAP_RELEASES.set(releases);
}

/// Blocks the heap has taken back; 0 where there is no heap.
pub fn heap_releases() -> u32 {
    (HEAP_RELEASES.get(|| no_releases))()
}

static HEAP_LIVE: Global<fn() -> usize> = Global::new();

/// Installs the reading a pressure test compares: the bytes handed out and not
/// yet given back.
pub fn set_heap_live(live: fn() -> usize) {
    HEAP_LIVE.set(live);
}

/// The bytes the heap has handed out and not taken back; 0 where there is none.
pub fn heap_live() -> usize {
    (HEAP_LIVE.get(|| no_heap))()
}

static HEAP_BYTES: Global<fn() -> usize> = Global::new();

/// Installs the size of the range the heap was given.
pub fn set_heap_bytes(bytes: fn() -> usize) {
    HEAP_BYTES.set(bytes);
}

/// The bytes of the range the heap was given; 0 where there is none.
pub fn heap_bytes() -> usize {
    (HEAP_BYTES.get(|| no_heap))()
}

static HEAP_REFUSALS: Global<fn() -> u32> = Global::new();

/// Installs the count of requests the heap answered with nothing: the one
/// way this memory is full, since the fixed ranges cannot be.
pub fn set_heap_refusals(refusals: fn() -> u32) {
    HEAP_REFUSALS.set(refusals);
}

/// Requests the heap refused since boot; 0 where there is no heap.
pub fn heap_refusals() -> u32 {
    (HEAP_REFUSALS.get(|| no_releases))()
}

static HEAP_LARGEST_FREE: Global<fn() -> usize> = Global::new();

/// Installs the size of the largest block on the heap's free lists — how
/// fragmented the reached part is, where live against used says how full.
pub fn set_heap_largest_free(largest: fn() -> usize) {
    HEAP_LARGEST_FREE.set(largest);
}

/// The largest block on the heap's free lists; 0 where there is none.
pub fn heap_largest_free() -> usize {
    (HEAP_LARGEST_FREE.get(|| no_heap))()
}
