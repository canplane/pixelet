// pxl-kernel/door.rs

//! The kernel's side of the doorbell, which a bracket (pxl-wasm, pxl-native)
//! hands each turn to: the directory, commands applied, signals raised. The
//! rings are xpute's (`xpute_runtime::ipc::doorbell`).

mod apply;
mod executor;
pub mod layout;

use crate::abi::cmd::{Command, Sensor, SensorStartSignal, WorldSnapshotSignal};
use crate::host::HeapStat;
use crate::io::load::LoadKind;
use crate::kernel::kernel;
use crate::mem::fixed::Range;
use crate::mem::section;
use layout::DOOR_LAYOUT;
use pxl_render::mesh::volume::Volumes;
use xpute_core::status::errno::Errno;
use xpute_core::wire::xtp::PacketWriter;
use xpute_runtime::global::Global;
use xpute_runtime::ipc::directory::{directory_words, quantum_entries, spec::key, write_directory, Entry};
use xpute_runtime::ipc::doorbell::Door;
use xpute_runtime::ipc::ring::Ring;
use xpute_runtime::mem::base::{off_of, off_of_slice, ptr_at};

pub struct Bracket {
    pub given: u32,
    /// End of the bracket's own data (stack and ring fronts below it); the kernel's ranges go past it.
    pub data_end: u32,
    pub clock: fn() -> f64,
    pub heap_stat: fn() -> HeapStat,
    /// Where a bracket whose process dies with the kernel reports why: no host
    /// is left to run the record, and a panic's text is not in its numbers.
    /// None where the host outlives the kernel (a page).
    pub last_words: Option<crate::host::LastWords>,
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
        unsafe { Door::init(ptr_at(0), &DOOR_LAYOUT, crate::sched::profiler::record::profiler_note_rings) }
    })
}

fn raise(cmd: Command, count: u32, f: impl Fn(&mut PacketWriter)) {
    door().raise(cmd as u32, count, f);
}

/// Where a ring's words start in the memory.
fn ring_address(ring: &Ring) -> u32 {
    off_of(ring.words().as_ptr()) as u32
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State { snapshot: 0, profiler_taken: false })
}

/// xpute's three (two rings and the quantum) count as five entries, plus the
/// host's stream, the GPU stream and the heap row.
const DIRECTORY_ENTRIES: usize = 8;

/// The bracket's `HeapStat` as words, written past the directory every turn
/// for the shell to read in place.
const HEAP_ROW_AT: u32 = crate::mem::spec::DIRECTORY_AT + directory_words(DIRECTORY_ENTRIES) as u32 * 4;
const HEAP_ROW_WORDS: usize = 9;

fn write_heap_row() {
    let h = crate::host::heap_stat();
    // SAFETY: the directory's page past the directory, the row's alone.
    let row = unsafe { &mut *ptr_at::<[u32; HEAP_ROW_WORDS]>(HEAP_ROW_AT as usize) };
    *row = [
        h.range as u32,
        h.used as u32,
        h.live as u32,
        h.run as u32,
        h.slot as u32,
        h.largest_free as u32,
        h.releases,
        h.refusals,
        h.requests,
    ];
}

/// A shell has booted. On a device the kernel outlives a JS reload, and the
/// old shell's reads, surfaces, sensor watches and text requests will never
/// be answered; a read left out keeps its credit forever, so all of it is
/// let go here.
fn host_attached(s: &mut State) {
    crate::io::sched::reset();
    let open: Vec<u32> = pxl_render::gpu::surface::surfaces().map(|surface| surface.handle).collect();
    for handle in open {
        if let Some(closed) = pxl_render::gpu::surface::surface_close(handle) {
            pxl_render::gpu::renderer::surface_closed(closed);
        }
    }
    crate::shell::shell::sensors_close();
    crate::text::text_forget();
    s.snapshot = 0;
}

/// First turn: lays out memory and writes the directory at DIRECTORY_AT.
fn boot(b: &Bracket) {
    xpute_runtime::clock::set_clock(b.clock);
    crate::host::set_heap_stat(b.heap_stat);
    crate::host::set_last_words(b.last_words);
    executor::install();
    // Named invariants report through `bug!`; a core panic reports only its location.
    xpute_core::status::bug::set_report(crate::host::host_bug);
    std::panic::set_hook(Box::new(|info| {
        if crate::host::stopped() {
            return;
        }
        crate::host::last_words(crate::host::Stop::Panic(info));
        if let Some(at) = info.location() {
            crate::host::host_bug(Errno::ENOTRECOVERABLE, at, 0, 0);
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
        (crate::abi::directory::key::HEAP_ROW, HEAP_ROW_AT),
    ];
    // SAFETY: the page at DIRECTORY_AT, kept clear of the rings and the
    // bracket's data; the directory's and the heap row's alone.
    let words = unsafe { &mut *ptr_at::<[u32; directory_words(DIRECTORY_ENTRIES)]>(crate::mem::spec::DIRECTORY_AT as usize) };
    write_directory(words, &entries);
}

fn fixed_at(r: Range) -> u32 {
    crate::mem::fixed::range(r).offset
}

/// Lays the memory map and makes the volumes. Returns where GPU calls are
/// recorded for the host, 0 where the kernel makes them itself.
fn lay_out(b: &Bracket) -> u32 {
    // The map first: the kernel's first use makes the chunk table, whose
    // tables lie in `.fixed`.
    section::set_map(section::lay_out(b.given, b.data_end));
    kernel().volumes = Some(Volumes::new());
    // SAFETY: `.fixed.gpu`, `.fixed.host` and `.fixed.blocks`, each what it
    // is installed as from a page, zeroed, the renderer's and the host's
    // alone.
    unsafe {
        crate::host::set_host_stream(fixed_at(Range::Host));
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
    crate::host::host_rise();
    crate::world::change::world_edge().rise(quota_ms);
    door().drain(|number, pkt| apply::apply(s, number, pkt));
    if quota_ms > 0.0 || world_frame_waiting() {
        xpute_runtime::sched::task::poll();
    }
    if take_world_drawn() {
        post_shell(s);
    }
    pxl_render::gpu::api::gpu_publish();
    write_heap_row();
    crate::host::host_fall();
    let wake = crate::world::change::world_fall();
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
        if let Some(len) = crate::shell::shell::ui_snapshot(buf) {
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
    // The key crosses as the one word the queue orders by; the shell holds it
    // as a handle and hands it back, and never reads into it.
    // A bake's object and a model are files the kernel opens; an image is
    // decoded where the device is, so it is asked for by command.
    take_started(|key, category, subject| match category.bake() {
        Some(_) => crate::io::fetch::start(key, category, subject),
        None => match crate::io::load::of_subject(subject) {
            Some((LoadKind::Gltf, load)) => crate::io::load::open_model(key, load),
            Some((LoadKind::Image, load)) => {
                let signal = LoadStartSignal { key, load: load.number() };
                raise(Command::LOAD_START, LoadStartSignal::FIELDS, |w| signal.write(w));
            }
            None => {}
        },
    });
    // Collected first: stopping a read settles it, which must not happen inside
    // the request scheduler's own walk.
    let mut stops = Vec::new();
    take_stops(|key| stops.push(key));
    for key in stops {
        if !crate::io::fetch::cancel(key) && !crate::io::load::cancel_model(key) {
            raise(Command::LOAD_CANCEL, LoadCancelSignal::FIELDS, |w| LoadCancelSignal { key }.write(w));
        }
    }
}
