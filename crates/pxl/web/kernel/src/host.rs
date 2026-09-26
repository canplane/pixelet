// pxl-kernel/host.rs
//
// What the kernel asks of the host that is not a command it answers: the
// browser's key-value storage, a body the host holds copied in, a glyph's
// advance. Each is a record in the host stream (abi/host.json), which the
// host runs once the turn has returned, so what a record asks is done by the
// time the next turn rises — and what it names is kept that long. Only the
// clock is still called: a turn measures its own budget mid-turn, when the
// host cannot run. A native test has no stream; storage keeps nothing there,
// and a glyph is a fixed width.

use core::panic::Location;
use std::collections::{HashMap, HashSet};

use pxl_render::gpu::paint::Font;
use xpute_core::status::errno::Errno;
use xpute_runtime::ipc::stream::Stream;
use xpute_runtime::mem::base::{off_of, ptr_at};

use crate::abi::host::{op, stream};
use crate::global::Global;

/// The host's clock, in milliseconds. The substrate holds it
/// (xpute clock.rs); it is named here because it arrives with the rest.
pub use xpute_runtime::clock::{now, set_clock};

struct Host {
    stream: Stream,
    /// What a record names, kept until the host has run it: a value to store.
    held: Vec<String>,
}

static HOST: Global<Option<Host>> = Global::new();

fn host() -> Option<&'static mut Host> {
    HOST.get(|| None).as_mut()
}

/// Installs the host stream at `at`, `stream::BYTES` of the kernel's memory
/// nothing else writes.
///
/// # Safety
/// `at` must address that range, 4-aligned, for as long as the module runs.
pub unsafe fn set_host_stream(at: u32) {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize), (stream::BYTES / 4) as usize) };
    HOST.set(Some(Host {
        stream: Stream::new(words, stream::HEAD as usize),
        held: Vec::new(),
    }));
}

/// The turn's rise: what the host did after the last turn is in, so what was
/// held for it goes and the advances it wrote are known.
pub fn host_rise() {
    *TURN.get(|| 0) += 1;
    if let Some(h) = host() {
        h.held.clear();
    }
    glyphs_rise();
}

static TURN: Global<u64> = Global::new();

/// Which turn this is. What a record asked for on turn `n` is done on any
/// turn after it.
pub fn turn() -> u64 {
    *TURN.get(|| 0)
}

/// The turn's fall: its records, for the host to run once it returns.
pub fn host_fall() {
    if let Some(h) = host() {
        h.stream.publish();
    }
}

/// A broken invariant (xpute-core status/bug.rs): its errno, its place and
/// its two numbers, as the one record of the turn, published at once for the
/// host to find when the trap reaches it. What was recorded before it in the
/// turn is dropped: the kernel stops here, and none of it is to be done.
pub fn host_bug(errno: Errno, at: &Location<'_>, a: u64, b: u64) {
    if let Some(h) = host() {
        h.stream.publish();
        let args = [errno as i32 as u32, at.line(), at.column(), a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32];
        h.stream.record(op::PANIC, &args, &[], Some(at.file().as_bytes()));
        h.stream.publish();
    }
}

/// `localStorage.setItem` under `storage`'s key (host.json).
pub fn storage_set(storage: u32, value: String) {
    let Some(h) = host() else { return };
    h.stream.record(op::STORAGE_SET, &[storage, off_of(value.as_ptr()) as u32, value.len() as u32], &[], None);
    h.held.push(value);
}

/// Asks the host to copy body `body` into `dst`, which is its length, and let
/// it go; an empty `dst` only lets it go. The bytes are there from the next
/// turn on, and `dst` must be kept until then.
pub fn body_read(body: u32, dst: &mut [u8]) {
    let Some(h) = host() else { return };
    h.stream.record(op::BODY_READ, &[body, off_of(dst.as_mut_ptr()) as u32, dst.len() as u32], &[], None);
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

/// Every glyph advance the host has said, by font and code point, and what
/// was asked for since the last turn and where its answers go. The table
/// grows with the characters a session has drawn in a font, which is bounded
/// by the fonts and the script, not by what is drawn.
struct Glyphs {
    known: HashMap<(u32, u32, u32), f32>,
    asked: HashSet<(u32, u32, u32)>,
    answers: Vec<Asked>,
}

/// One MEASURE record's glyphs, and where the host writes their advances.
struct Asked {
    weight: u32,
    px: u32,
    cps: Vec<u32>,
    widths: Box<[f32]>,
}

static GLYPHS: Global<Glyphs> = Global::new();

fn glyphs() -> &'static mut Glyphs {
    GLYPHS.get(|| Glyphs {
        known: HashMap::new(),
        asked: HashSet::new(),
        answers: Vec::new(),
    })
}

fn glyphs_rise() {
    let g = glyphs();
    if g.answers.is_empty() {
        return;
    }
    for a in g.answers.drain(..) {
        for (cp, w) in a.cps.into_iter().zip(a.widths.iter()) {
            g.known.insert((a.weight, a.px, cp), *w);
        }
    }
    g.asked.clear();
    // What was waiting on a width is laid out on the frame that knows it.
    crate::world::change::request_world_tick();
}

/// The width `text` takes in `font`: the sum of its glyphs' advances, as the
/// host's canvas measures each. None while one is not yet known, and it is
/// asked for: the next turn knows it.
pub fn measure_text(font: Font, text: &str) -> Option<f64> {
    let Some(h) = host() else { return Some(text.chars().count() as f64 * 7.0) };
    let g = glyphs();
    let px = font.px.to_bits();
    let mut width = 0.0;
    let mut missing = Vec::new();
    for c in text.chars() {
        let key = (font.weight, px, c as u32);
        match g.known.get(&key) {
            Some(&w) => width += w as f64,
            None => {
                if g.asked.insert(key) {
                    missing.push(c as u32);
                }
            }
        }
    }
    if !missing.is_empty() {
        let mut widths = vec![0.0f32; missing.len()].into_boxed_slice();
        h.stream.record(op::MEASURE, &[font.weight, px, off_of(widths.as_mut_ptr()) as u32], &missing, None);
        g.answers.push(Asked {
            weight: font.weight,
            px,
            cps: missing,
            widths,
        });
    }
    text.chars().all(|c| g.known.contains_key(&(font.weight, px, c as u32))).then_some(width)
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

/// The module's heap as it stands; every word 0 where there is none.
pub fn heap_stat() -> HeapStat {
    (HEAP.get(|| no_heap))()
}
