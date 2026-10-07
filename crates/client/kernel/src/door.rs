// pxl-kernel/door.rs

//! The kernel's side of the doorbell, which a bracket (pxl-wasm, pxl-native)
//! hands each turn to: the directory, commands applied, signals raised. The
//! rings are xpute's (`xpute_guest::ipc::door`).

mod apply;
mod executor;
pub mod host;
pub mod layout;

use crate::abi::cmd::{Command, Sensor, SensorStartSignal, WorldSnapshotSignal};
use crate::door::host::HeapStat;
use crate::kernel;
use crate::mem::fixed::Range;
use crate::mem::section;
use layout::DOOR_LAYOUT;
use xpute_core::status::errno::Errno;
use xpute_core::wire::xtp::PacketWriter;
use xpute_guest::global::Global;
use xpute_guest::ipc::directory::{directory_words, quantum_entries, spec::key, write_directory, Entry};
use xpute_guest::ipc::door::Door;
use xpute_guest::ipc::ring::Ring;
use xpute_guest::mem::base::{off_of, off_of_slice, ptr_at};

pub struct Bracket {
    pub given: u32,
    /// End of the bracket's own data (stack and ring fronts below it); the kernel's ranges go past it.
    pub data_end: u32,
    pub clock: fn() -> f64,
    pub heap_stat: fn() -> HeapStat,
    /// Where a bracket whose process dies with the kernel reports why: no host
    /// is left to run the record, and a panic's text is not in its numbers.
    /// None where the host outlives the kernel (a page).
    pub last_words: Option<crate::door::host::LastWords>,
}

struct State {
    /// Hash of the last posted UI snapshot; one is posted only when it changed.
    snapshot: u64,
    /// Console rows were handed over, to be let go next turn.
    profiler_taken: bool,
}

static DOOR: Global<Door> = Global::new();

fn door() -> &'static Door {
    DOOR.get(|| {
        // SAFETY: the module's memory, which never moves and outlives the door;
        // the ranges are the layout's alone.
        unsafe { Door::init(ptr_at(0), &DOOR_LAYOUT, crate::param::MOST_SIGNALS, crate::sched::profiler::record::profiler_note_rings) }
    })
}

fn raise(cmd: Command, count: u32, f: impl Fn(&mut PacketWriter)) {
    door().raise(cmd as u32, count, f);
}

fn ring_address(ring: &Ring) -> u32 {
    off_of(ring.words().as_ptr()) as u32
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State { snapshot: 0, profiler_taken: false })
}

/// xpute's three (two rings and the quantum) count as five entries, plus the
/// host's stream and the GPU stream.
const DIRECTORY_ENTRIES: usize = 7;

/// A shell has booted. On a device the kernel outlives a JS reload, and the
/// old shell's reads, surfaces, sensor watches and text requests will never
/// be answered; a read left out keeps its credit forever, so all of it is
/// let go here.
/// A case lives on its surface, so it goes with it: a preview never closed
/// held its actor for good.
fn close_surface(handle: u32) {
    if let Some(closed) = pxl_render::gpu::surface::surface_close(handle) {
        pxl_render::gpu::renderer::surface_closed(closed);
    }
    crate::scene::character::preview::preview_close(handle);
}

fn host_attached(s: &mut State) {
    crate::io::sched::reset();
    let open: Vec<u32> = pxl_render::gpu::surface::surfaces().map(|surface| surface.handle).collect();
    for handle in open {
        close_surface(handle);
    }
    crate::shell::sensors_close();
    crate::io::text::text_forget();
    s.snapshot = 0;
}

/// First turn: lays out memory and writes the directory at DIRECTORY_AT.
fn boot(b: &Bracket) {
    xpute_guest::clock::set_clock(b.clock);
    crate::door::host::set_heap_stat(b.heap_stat);
    crate::door::host::set_last_words(b.last_words);
    executor::install();
    // Named invariants report through `bug!`; a core panic reports only its location.
    xpute_core::status::bug::set_report(crate::door::host::host_bug);
    std::panic::set_hook(Box::new(|info| {
        if crate::door::host::stopped() {
            return;
        }
        crate::door::host::last_words(crate::door::host::Stop::Panic(info));
        if let Some(at) = info.location() {
            crate::door::host::host_bug(Errno::ENOTRECOVERABLE, at, 0, 0);
        }
    }));
    let gpu_stream = lay_out(b);
    crate::sched::turn::boot();
    let [margin, batch, settle] = quantum_entries(&crate::sched::tick::QUANTUM);
    let entries: [Entry; DIRECTORY_ENTRIES] = [
        (key::SUBMISSION, ring_address(door().submission())),
        (key::COMPLETION, ring_address(door().completion())),
        margin,
        batch,
        settle,
        (key::SYS, fixed_at(Range::Host)),
        (crate::abi::directory::key::GPU_STREAM, gpu_stream),
    ];
    // SAFETY: the page at DIRECTORY_AT, kept clear of the rings and the
    // bracket's data; the directory's alone.
    let words = unsafe { &mut *ptr_at::<[u32; directory_words(DIRECTORY_ENTRIES)]>(crate::mem::spec::DIRECTORY_AT as usize) };
    write_directory(words, &entries);
}

fn fixed_at(r: Range) -> u32 {
    crate::mem::fixed::range(r).offset
}

/// Lays the memory map. Returns where GPU calls are
/// recorded for the host, 0 where the kernel makes them itself.
fn lay_out(b: &Bracket) -> u32 {
    // The map first: the kernel's first use makes the chunk table, which lives in `.fixed`.
    section::set_map(section::lay_out(b.given, b.data_end));
    kernel().laid_out = true;
    // SAFETY: `.fixed.gpu`, `.fixed.host` and `.fixed.blocks`, zeroed, each the
    // renderer's or the host's alone.
    unsafe {
        crate::door::host::set_host_stream(fixed_at(Range::Host));
        pxl_render::gpu::renderer::set_blocks(fixed_at(Range::Blocks), crate::mem::fixed::BLOCK_SLOTS);
        pxl_render::gpu::api::install(fixed_at(Range::Gpu))
    }
}

/// A turn: applies waiting submissions, polls tasks on the quota, and
/// returns when to wake next, in ms (0 for the next frame, NO_WAKE for
/// none). The first turn boots.
pub fn interrupt(quota_ms: f64, bracket: fn() -> Bracket) -> f64 {
    use crate::sched::turn::{take_world_drawn, world_frame_waiting};
    if section::map().is_none() {
        boot(&bracket());
    }
    let s = state();
    if core::mem::take(&mut s.profiler_taken) {
        crate::sched::profiler::log::profiler_log_clear();
    }
    crate::door::host::host_rise();
    crate::sched::edge::world_edge().rise(quota_ms);
    door().drain(|number, pkt| apply::apply(s, number, pkt));
    if quota_ms > 0.0 || world_frame_waiting() {
        xpute_guest::sched::task::poll();
    }
    if take_world_drawn() {
        post_shell(s);
    }
    pxl_render::gpu::api::gpu_publish();
    crate::door::host::host_fall();
    let wake = crate::sched::edge::world_fall();
    // What the completion ring had no room for is still in the submission ring.
    if door().pending() {
        0.0
    } else {
        wake
    }
}

/// Grows to the largest snapshot and stays, so a settled turn allocates nothing.
/// The shell reads it on the signal, before the next turn can rewrite it.
static SNAPSHOT: Global<Vec<u8>> = Global::new();

fn post_shell(s: &mut State) {
    use crate::control::sensor::device_orientation::{take_orientation_commands, OrientationCommand};
    use crate::control::sensor::geolocation::{take_geolocation_commands, GeolocationCommand};
    use crate::io::sched::{take_started, take_stops};

    let sensor = |cmd: Command, sensor: Sensor| raise(cmd, SensorStartSignal::FIELDS, |w| SensorStartSignal { sensor }.write(w));
    for c in take_geolocation_commands() {
        match c {
            GeolocationCommand::Start => sensor(Command::SENSOR_START, Sensor::Geolocation),
            GeolocationCommand::Stop => sensor(Command::SENSOR_STOP, Sensor::Geolocation),
        }
    }
    for c in take_orientation_commands() {
        match c {
            OrientationCommand::Listen => sensor(Command::SENSOR_START, Sensor::Orientation),
            OrientationCommand::Remove => sensor(Command::SENSOR_STOP, Sensor::Orientation),
        }
    }
    let buf = SNAPSHOT.get(Vec::new);
    let len = loop {
        if let Some(len) = crate::shell::ui_snapshot(buf) {
            break len;
        }
        let room = (buf.len() * 2).max(section::PAGE_BYTES as usize);
        buf.resize(room, 0);
    };
    let mut h = fnv::FnvHasher::default();
    std::hash::Hasher::write(&mut h, &buf[..len as usize]);
    let hash = std::hash::Hasher::finish(&h);
    if hash != s.snapshot {
        s.snapshot = hash;
        let signal = WorldSnapshotSignal {
            at: off_of_slice(&raw const buf[..]) as u32,
            len,
        };
        raise(Command::WORLD_SNAPSHOT, WorldSnapshotSignal::FIELDS, |w| signal.write(w));
    }
    // The key is an opaque handle to the shell.
    take_started(|credit, category, subject| match category.bake() {
        Some(_) => crate::io::fetch::start(credit, category, subject),
        None => {
            if let Some((kind, load)) = crate::io::load::of_subject(subject) {
                crate::io::load::open_asset(credit, kind, load);
            }
        }
    });
    // Collected first: stopping a read settles it, which must not happen inside
    // the request scheduler's own walk.
    let mut stops = Vec::new();
    take_stops(|key| stops.push(key));
    for key in stops {
        if !crate::io::fetch::cancel(key) {
            crate::io::load::cancel_asset(key);
        }
    }
}
