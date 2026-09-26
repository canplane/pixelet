// pxl-kernel/door.rs

//! The kernel's side of the doorbell: what a bracket — pxl-wasm in a page,
//! pxl-native in a device app — hands a turn to. The rings and their order
//! are xpute's (xpute_runtime::ipc::doorbell); what is here is the kernel's
//! part of them: the boot block, the commands applied, the signals raised and
//! the profiler read in place, the same on every host since the shell on the
//! other side is the same. What a bracket supplies is only what its platform
//! decides (`Bracket`).
//!
//! The kernel is single-threaded, so the state is a plain global.

mod apply;
mod executor;
pub mod layout;

use crate::abi::cmd::{Command, FetchCreateSignal, FetchDeleteSignal, LoadCreateSignal, Sensor, SensorCreateSignal, WorldSnapshotSignal, WorldUpdateSignal};
use crate::global::Global;
use crate::host::HeapStat;
use crate::kernel::kernel;
use crate::mem::fixed::Range;
use crate::mem::section;
use layout::{RING_CAPACITY, RING_DESC_AT, RING_DESC_STRIDE, RING_SLOT_AT, RING_SLOT_BYTES, RING_SLOT_STRIDE};
use pxl_render::mesh::volume::Volumes;
use xpute_core::math::scalar::round;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_core::wire::xtp::PacketWriter;
use xpute_runtime::ipc::doorbell::{Door, DoorLayout};
use xpute_runtime::ipc::ring::Ring;
use xpute_runtime::mem::base::{off_of, off_of_slice, ptr_at};

/// What a bracket gives the kernel on its first turn.
pub struct Bracket {
    /// The memory's bytes from its base: what the kernel lays its map over.
    pub given: u32,
    /// Where the bracket's own data ends in the memory, the stack and the
    /// rings' front below it: the kernel places its ranges past it.
    pub data_end: u32,
    /// The client's clock, in milliseconds.
    pub clock: fn() -> f64,
    /// The allocator's reading, for the profiler and the heap's row.
    pub heap_stat: fn() -> HeapStat,
    /// Where a bracket whose process stops with the kernel says what stopped
    /// it: no host is left to run the report's record, and a panic's text —
    /// a wgpu validation error's, say — is not in its numbers. None where the
    /// host outlives the kernel and runs the record (a page).
    pub last_words: Option<crate::host::LastWords>,
}

struct State {
    /// The UI snapshot last posted, by hash: one is posted only when it changed.
    snapshot: u64,
    /// The console stream's rows were handed over, to be let go on the next
    /// turn (PROFILER_TAKE).
    profiler_taken: bool,
}

/// The rings, laid out empty: the whole memory is what a ring's two areas are
/// cut from, the descriptors near the bottom and the slots' payloads past
/// them, at addresses the layout fixes rather than anyone hands out.
static DOOR: Global<Door> = Global::new();

fn door() -> &'static Door {
    DOOR.get(|| {
        let layout = DoorLayout {
            capacity: RING_CAPACITY,
            slot_bytes: RING_SLOT_BYTES,
            desc_at: RING_DESC_AT,
            desc_stride: RING_DESC_STRIDE,
            slot_at: RING_SLOT_AT,
            slot_stride: RING_SLOT_STRIDE,
        };
        // SAFETY: the module's memory, which never moves and outlives the
        // door; the ranges are the layout's and no one else's.
        unsafe { Door::init(ptr_at(0), &layout, crate::sched::profiler::record::profiler_note_rings) }
    })
}

/// A signal for the shell with a packet of `count` children written by `f`.
fn raise(cmd: Command, count: u32, f: impl Fn(&mut PacketWriter)) {
    door().raise(cmd as u32, count, f);
}

/// Where a ring's words start in the memory.
fn ring_address(ring: &Ring) -> u32 {
    off_of(ring.words().as_ptr()) as u32
}

/// The door's one state, made on first use. xpute's device for it rather
/// than a `static mut`, whose references the compiler now warns of: the kernel
/// is single-threaded and no turn re-enters another, which is the promise
/// `Global` is built on.
static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State { snapshot: 0, profiler_taken: false })
}

// ---- Rings ----
//
// Commands and their effects cross in rings (crates/xpute/runtime/src/ipc/ring.rs):
// a message names a command (crate::abi::cmd) and an XTP packet in the
// ring's slot, which the kernel reads where it lies. The shell assembles
// what it submits and takes apart what completes; nothing here is shaped
// for it. The shell pushes into the submission ring, rings `interrupt` with
// the turn's quota, and reads the completion ring when the call returns.

/// Words in the boot block, each one the shell reads: where the submission
/// ring and the completion ring are; the quantum the kernel declares to the
/// central scheduler (xpute_runtime::sched::quantum's QuantumPolicy): margin
/// share, batch frames and the settle time, each in thousandths; the heap's
/// row; and the two streams the shell runs after every turn, the renderer's
/// (pxl_render::gpu::api, 0 where the bracket draws itself) and the host's
/// (crate::host). Where the kernel's ranges lie is its own and is not here:
/// what the shell reads of them, it is given the address of.
pub const BOOT_WORDS: usize = 8;

const BOOT_QUANTUM: usize = 2;

/// The boot word holding the address of the heap's row.
const BOOT_HEAP: usize = 5;

/// The boot words holding the addresses of the renderer's and the host's
/// streams.
const BOOT_GPU: usize = 6;
const BOOT_HOST: usize = 7;

/// The heap's row: the bracket's reading (`HeapStat`) as words — range, used,
/// live, run, slot, largest free, releases, refusals, requests — written at
/// every turn's fall past the boot block, in the memory, where the shell reads
/// it in place.
const HEAP_ROW_AT: u32 = crate::mem::spec::BOOT_AT + BOOT_WORDS as u32 * 4;
const HEAP_ROW_WORDS: usize = 9;

fn write_heap_row() {
    let h = crate::host::heap_stat();
    // SAFETY: the boot page past the boot block, the row's alone.
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

/// A shell has booted against this kernel. On a page there is only ever the
/// first, since the page's module goes with its shell; on a device the kernel
/// outlives a reload of the JavaScript, and what the shell before was running
/// — its reads and loads, its surfaces, its sensors' watches, the texts it
/// was asked to lay out — is answered by no one now. A read left out keeps
/// its credit until it is answered, so a reload with every credit out asked
/// for nothing ever again. All of it is let go here, and the new shell starts
/// from what it asks for.
fn host_attached(s: &mut State) {
    crate::io::sched::reset();
    let open: Vec<u32> = pxl_render::gpu::surface::surfaces().map(|surface| surface.handle).collect();
    for handle in open {
        if let Some(closed) = pxl_render::gpu::surface::surface_delete(handle) {
            pxl_render::gpu::renderer::surface_closed(closed);
        }
    }
    crate::shell::shell::sensors_close();
    crate::text::text_forget();
    s.snapshot = 0;
}

/// The first turn's: lays out the memory the module was given and writes the
/// boot block where mem.json's BOOT_AT says, for the shell to read once
/// before any ring.
fn boot(b: &Bracket) {
    crate::host::set_clock(b.clock);
    crate::host::set_heap_stat(b.heap_stat);
    crate::host::set_last_words(b.last_words);
    executor::install();
    // A broken invariant the kernel names reports itself (`bug!`); one core
    // raises — an index past a slice, an `unwrap` of nothing — reports its
    // place alone, the errno being that no invariant said why.
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
    // SAFETY: the page at BOOT_AT, which the layout keeps clear of the rings
    // and the bracket's data; the boot block's alone.
    let block = unsafe { &mut *ptr_at::<[u32; BOOT_WORDS]>(crate::mem::spec::BOOT_AT as usize) };
    block[..2].copy_from_slice(&[ring_address(door().submission()), ring_address(door().completion())]);
    block[BOOT_HEAP] = HEAP_ROW_AT;
    block[BOOT_GPU] = gpu_stream;
    block[BOOT_HOST] = fixed_at(Range::Host);
    let q = crate::sched::tick::QUANTUM;
    for (i, v) in [q.margin_share, q.batch_frames, q.settle_ms].into_iter().enumerate() {
        block[BOOT_QUANTUM + i] = round(v * 1000.0) as u32;
    }
}

/// Where fixed range `r` begins, the map laid.
fn fixed_at(r: Range) -> u32 {
    crate::mem::fixed::range(r).or_bug(Errno::ENOTRECOVERABLE).offset
}

/// Records the kernel's map over the memory the bracket gave, and makes the
/// volumes over it; with it, where the renderer's calls are recorded for the
/// host to run, 0 where the kernel makes them itself (pxl_render::gpu::api).
///
/// Nothing is cut here. The sections are ranges the map names and their
/// contents' own affair; the heap is the one allocator and it knows its range
/// without being told (`buddy_tree`'s `Range`).
fn lay_out(b: &Bracket) -> u32 {
    let map = section::lay_out(b.given, b.data_end);
    kernel().volumes = Some(Volumes::new());
    section::set_map(map);
    // SAFETY: `.fixed.gpu`, `.fixed.host` and `.fixed.blocks`, each what it
    // is installed as from a page, zeroed, the renderer's and the host's
    // alone.
    unsafe {
        crate::host::set_host_stream(fixed_at(Range::Host));
        pxl_render::gpu::renderer::set_blocks(fixed_at(Range::Blocks));
        pxl_render::gpu::api::install(fixed_at(Range::Gpu))
    }
}

/// The doorbell: the rising edge of a turn, with the time it may take. The
/// kernel applies the submissions waiting, then polls its tasks on the quota
/// — the frame among them, when the host asked for one — and the return is
/// the falling edge: when the kernel wants the next turn, in ms from now, 0
/// for the next frame and xpute_runtime::sched::quantum's NO_WAKE for none. A
/// turn rung with no quota and no frame only applies what waits. What the
/// kernel asked of the host in the turn is in the two streams by then, for
/// the shell to run before anything else can reach the device or the next
/// turn can begin. The first turn boots, over what `bracket` gives.
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
        post_turn(s);
    }
    pxl_render::gpu::api::gpu_publish();
    write_heap_row();
    crate::host::host_fall();
    crate::world::change::world_fall()
}

/// The UI's snapshot, a Snapshot packet written in place turn after turn:
/// what WORLD_SNAPSHOT names instead of carrying. The shell reads it when it
/// hears the signal, before the next turn can write it again, so there is no
/// block to hand back. It grows to the largest snapshot and stays there, so
/// a settled turn asks the heap for nothing.
static SNAPSHOT: Global<Vec<u8>> = Global::new();

/// What the kernel asks of the shell since it last asked, raised as signals:
/// the sensors' watches, console lines, the UI's snapshot when it
/// changed, the fetches to start and cancel, and the loads.
fn post_shell(s: &mut State) {
    use crate::control::sensor::device_orientation::{take_orientation_commands, OrientationCommand};
    use crate::control::sensor::geolocation::{take_geolocation_commands, GeolocationCommand};
    use crate::io::sched::{take_started, take_stops};

    let sensor = |cmd: Command, sensor: Sensor| raise(cmd, SensorCreateSignal::FIELDS, |w| SensorCreateSignal { sensor }.write(w));
    for c in take_geolocation_commands() {
        match c {
            GeolocationCommand::Start => sensor(Command::SENSOR_CREATE, Sensor::Geolocation),
            GeolocationCommand::Stop => sensor(Command::SENSOR_DELETE, Sensor::Geolocation),
        }
    }
    for c in take_orientation_commands() {
        match c {
            OrientationCommand::Listen => sensor(Command::SENSOR_CREATE, Sensor::Orientation),
            OrientationCommand::Remove => sensor(Command::SENSOR_DELETE, Sensor::Orientation),
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
    take_started(|key, category, subject| match category.bake() {
        Some(bake) => {
            let name = crate::io::fetch::DemandName(category, subject);
            raise(Command::FETCH_CREATE, FetchCreateSignal::FIELDS, |w| FetchCreateSignal { key, bake, name: &name }.write(w));
        }
        // An asset belongs to no bake: it is named by its content, and the
        // host is the one that knows what that is.
        None => {
            let Some((kind, load)) = crate::io::load::of_subject(subject) else { return };
            let signal = LoadCreateSignal { key, kind, load: load.number() };
            raise(Command::LOAD_CREATE, LoadCreateSignal::FIELDS, |w| signal.write(w));
        }
    });
    take_stops(|key| raise(Command::FETCH_DELETE, FetchDeleteSignal::FIELDS, |w| FetchDeleteSignal { key }.write(w)));
}

/// The end of a frame, the kernel having drawn it: what it asks of the shell,
/// and last the draws it listed.
fn post_turn(s: &mut State) {
    post_shell(s);
    let signal = WorldUpdateSignal {
        draws: kernel().sched.draw.draw_count(),
    };
    raise(Command::WORLD_UPDATE, WorldUpdateSignal::FIELDS, |w| signal.write(w));
}
