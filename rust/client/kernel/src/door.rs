// pxl-kernel/door.rs

//! The kernel's side of the doorbell, which a bracket (pxl-wasm, pxl-native)
//! hands each turn to: the directory, commands applied, signals raised. The
//! rings are xpute's (`xpute_guest::ipc::door`).

mod apply;
pub(crate) use apply::over_marker;
/// The directory's keys, generated from directory.json.
#[path = "door/directory.spec.rs"]
pub mod directory;
mod executor;
pub mod layout;

use crate::domain::control::cmd::{Command, Sensor, SensorStartSignal, WorldSnapshotSignal};
use crate::profiler::record::profiler_record_turn;
use crate::profiler::spec::Wake;
use crate::sys::mem::section;
use layout::DOOR_LAYOUT;
use xpute_guest::clock::now;
use xpute_guest::global::{off_marked_thread, Global};
use xpute_guest::ipc::directory::{directory_words, quantum_entries, spec::key, write_directory, Entry};
use xpute_guest::ipc::door::{Door, DoorStat};
use xpute_guest::ipc::ring::Ring;
use xpute_guest::mem::base::{off_of, off_of_slice, ptr_at};
use xpute_guest::mem::heap::HeapStat;
use xpute_kit::collection::list::List;
use xpute_kit::status::errno::Errno;
use xpute_kit::wire::xtp::PacketWriter;

pub struct Bracket {
    pub given: u32,
    /// End of the bracket's own data (stack and ring fronts below it); the kernel's ranges go past it.
    pub data_end: u32,
    pub clock: fn() -> f64,
    pub heap_stat: fn() -> HeapStat,
    /// Whether the surfaces take a frame now without blocking; one admitted
    /// is drawn this turn, and the bracket counts it until the GPU has done
    /// it. A page's always: the browser paces its frames. A device's while
    /// fewer than two are unfinished, since taking a drawable there blocks
    /// the thread that waits for the turn.
    pub admit_frame: fn() -> bool,
    /// Where a bracket whose process dies with the kernel reports why: no host
    /// is left to run the record, and a panic's text is not in its numbers.
    /// None where the host outlives the kernel (a page).
    pub last_words: Option<crate::sys::io::host::LastWords>,
}

struct State {
    /// The UI snapshot last posted; one is posted only where it moved.
    sent: crate::shell::Sent,
    /// A UI command was applied this turn, so its falling edge posts to the shell.
    asked: bool,
}

static DOOR: Global<Door> = Global::new();

fn door() -> &'static Door {
    DOOR.get(|| {
        // SAFETY: the module's memory, which never moves and outlives the door;
        // the ranges are the layout's alone.
        unsafe { Door::init(ptr_at(0), &DOOR_LAYOUT, SIGNALS) }
    })
}

/// The rings' peaks since the last take, the profiler's (profiler/report.rs
/// `profiler_frame`).
pub(crate) fn take_stat() -> DoorStat {
    door().take_stat()
}

/// What a turn raises, all after its drain and once (`post_shell` at the
/// falling edge): a sensor's start or stop each, and the snapshot. A command
/// raises nothing itself, so the door's reserve past this is its reply.
const SIGNALS: u32 = Sensor::ALL.len() as u32 + 1;

// The shell reads the completion ring dry before it rings, so a ring with
// room for a reply to every submission and the door's reserve past the last
// (`Door::init`, SIGNALS + 1) never stops a drain partway (mem.json
// COMPLETION_CAPACITY).
const _: () = assert!(
    DOOR_LAYOUT.submission.capacity + SIGNALS < DOOR_LAYOUT.completion.capacity,
    "a full submission ring's replies and the turn's signals do not fit the completion ring"
);

fn raise(cmd: Command, count: u32, f: impl Fn(&mut PacketWriter)) {
    door().raise(cmd as u32, count, f);
}

fn ring_address(ring: &Ring) -> u32 {
    off_of(ring.words().as_ptr()) as u32
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        sent: Default::default(),
        asked: false,
    })
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
    crate::domain::scene::character::preview::preview_close(handle);
}

fn host_attached(s: &mut State) {
    crate::sys::io::request::reset();
    let open: Vec<u32> = pxl_render::gpu::surface::surfaces().map(|surface| surface.handle).collect();
    for handle in open {
        close_surface(handle);
    }
    crate::shell::sensors_close();
    crate::sys::io::text::text_forget();
    s.sent = Default::default();
}

/// First turn: lays out memory and writes the directory at DIRECTORY_AT.
fn boot(b: &Bracket) {
    xpute_guest::clock::set_clock(b.clock);
    crate::sys::io::host::set_heap_stat(b.heap_stat);
    crate::sys::io::host::set_admit_frame(b.admit_frame);
    crate::sys::io::host::set_last_words(b.last_words);
    executor::install();
    // Named invariants report through `bug!`; a core panic reports only its
    // location. Both are process-wide, and on a device the app's other
    // threads (a driver's callback, an image's decode) fail through them
    // too: those are not the kernel's to report, and its state is its own
    // thread's. Such a panic is told by the bracket's last words where it
    // has them, since they are where a device's reports are read (Android's
    // log), and by the hook this one replaced only where it has none, so
    // std's own hook does not print it a second time; a bug there is not
    // reported, and is told as the panic it then is (xpute_guest::global
    // `off_marked_thread`).
    xpute_kit::status::bug::set_report(|errno, at, a, b| {
        if !off_marked_thread() {
            crate::sys::io::host::host_bug(errno, at, a, b);
        }
    });
    let theirs = std::panic::take_hook();
    let say = b.last_words;
    std::panic::set_hook(Box::new(move |info| {
        if off_marked_thread() {
            match say {
                Some(say) => say(crate::sys::io::host::Stop::Panic(info)),
                None => theirs(info),
            }
            return;
        }
        if crate::sys::io::host::stopped() {
            return;
        }
        crate::sys::io::host::last_words(crate::sys::io::host::Stop::Panic(info));
        if let Some(at) = info.location() {
            crate::sys::io::host::host_bug(Errno::ENOTRECOVERABLE, at, 0, 0);
        }
    }));
    let gpu_stream = lay_out(b);
    crate::turn::boot();
    let [margin, batch, settle] = quantum_entries(&crate::sys::sched::tick::QUANTUM);
    let entries: [Entry; DIRECTORY_ENTRIES] = [
        (key::SUBMISSION, ring_address(door().submission())),
        (key::COMPLETION, ring_address(door().completion())),
        margin,
        batch,
        settle,
        (key::SYS, fixed_at(layout::Fixed::Host)),
        (crate::door::directory::key::GPU_STREAM, gpu_stream),
    ];
    // SAFETY: the page at DIRECTORY_AT, kept clear of the rings and the
    // bracket's data; the directory's alone.
    let words = unsafe { &mut *ptr_at::<[u32; directory_words(DIRECTORY_ENTRIES)]>(crate::sys::mem::spec::DIRECTORY_AT as usize) };
    write_directory(words, &entries);
}

fn fixed_at(f: layout::Fixed) -> u32 {
    layout::range(f).offset
}

/// Lays the memory map. Returns where GPU calls are
/// recorded for the host, 0 where the kernel makes them itself.
fn lay_out(b: &Bracket) -> u32 {
    // The map first: what lives in `.fixed` is handed its range from it.
    section::set_map(section::lay_out(b.given, b.data_end, layout::FIXED_BYTES));
    // Each range to its owner.
    crate::domain::layer::terrain::fixed::RANGE.set(layout::range(layout::Fixed::Terrain));
    crate::domain::layer::division::RANGE.set(layout::range(layout::Fixed::Division));
    // A layer asks for no more tiles than its residency frames.
    crate::sys::io::request::open(|c| crate::domain::view::demand::of_stream(c).map_or(0, |l| l.frames));
    // SAFETY: `.fixed.gpu`, `.fixed.host` and `.fixed.blocks`, zeroed, each the
    // renderer's or the host's alone.
    unsafe {
        crate::sys::io::host::set_host_stream(fixed_at(layout::Fixed::Host));
        pxl_render::gpu::block::set_blocks(fixed_at(layout::Fixed::Blocks), layout::BLOCK_SLOTS);
        pxl_render::gpu::api::install(fixed_at(layout::Fixed::Gpu))
    }
}

/// A turn: applies waiting submissions, polls tasks on the quota, and
/// returns when to wake next, in ms (0 for the next frame, NO_WAKE for
/// none). The first turn boots.
pub fn interrupt(quota_ms: f64, bracket: fn() -> Bracket) -> f64 {
    use crate::turn::{take_world_drawn, world_frame_waiting};
    if section::map().is_none() {
        boot(&bracket());
    }
    let s = state();
    let rise = now();
    crate::sys::io::host::host_rise();
    crate::sys::sched::edge::world_edge().rise(quota_ms);
    // The device's loss, as its host wrote it into the device's block: before
    // the commands, as a device answering again among them asks that block anew.
    if pxl_render::gpu::api::take_device_lost() {
        pxl_render::gpu::renderer::gpu_lost();
        crate::sys::io::loader::images_lost();
    }
    door().drain(|number, pkt| apply::apply(s, number, pkt));
    let drained = now();
    if quota_ms > 0.0 || world_frame_waiting() {
        xpute_guest::sched::task::poll();
    }
    let polled = now();
    // Once a turn, where a frame was drawn or a UI command applied: a second
    // snapshot would point into the buffer the first one's signal still names.
    let asked = core::mem::take(&mut s.asked);
    if take_world_drawn() || asked {
        post_shell(s);
    }
    pxl_render::gpu::api::gpu_publish();
    crate::sys::io::host::host_fall();
    let fall = crate::sys::sched::edge::world_fall();
    // What the completion ring had no room for is still in the submission ring.
    let wake = if door().pending() { 0.0 } else { fall };
    let asked = match wake {
        0.0 => Wake::next,
        w if w < 0.0 => Wake::none,
        _ => Wake::timer,
    };
    let fell = now();
    profiler_record_turn(fell - rise, polled - drained, f64::from(asked as u8), wake.max(0.0));
    wake
}

/// Grows to the largest snapshot and stays, so a settled turn allocates nothing.
/// One is posted a turn, and the shell reads it on the signal, before the next
/// turn can rewrite it.
static SNAPSHOT: Global<Vec<u8>> = Global::new();

fn post_shell(s: &mut State) {
    use crate::domain::control::sensor::device_orientation::{take_orientation_command, OrientationCommand};
    use crate::domain::control::sensor::geolocation::{take_geolocation_command, GeolocationCommand};
    use crate::sys::io::request::{take_started, take_stops};

    let sensor = |cmd: Command, sensor: Sensor| raise(cmd, SensorStartSignal::FIELDS, |w| SensorStartSignal { sensor }.write(w));
    if let Some(c) = take_geolocation_command() {
        match c {
            GeolocationCommand::Start => sensor(Command::SENSOR_START, Sensor::Geolocation),
            GeolocationCommand::Stop => sensor(Command::SENSOR_STOP, Sensor::Geolocation),
        }
    }
    if let Some(c) = take_orientation_command() {
        match c {
            OrientationCommand::Listen => sensor(Command::SENSOR_START, Sensor::Orientation),
            OrientationCommand::Remove => sensor(Command::SENSOR_STOP, Sensor::Orientation),
        }
    }
    if let Some(shown) = s.sent.moved() {
        let buf = SNAPSHOT.get(Vec::new);
        let len = loop {
            if let Some(len) = crate::shell::ui_snapshot(&shown, buf) {
                break len;
            }
            let room = (buf.len() * 2).max(xpute_guest::mem::section::PAGE_BYTES as usize);
            buf.resize(room, 0);
        };
        let signal = WorldSnapshotSignal {
            at: off_of_slice(&raw const buf[..]) as u32,
            len,
        };
        raise(Command::WORLD_SNAPSHOT, WorldSnapshotSignal::FIELDS, |w| signal.write(w));
    }
    // The key is an opaque handle to the shell.
    take_started(|credit, stream, subject| match stream.bake() {
        Some(_) => crate::sys::io::fetch::start(credit, stream, subject),
        None => {
            if let Some((_, load)) = crate::sys::io::loader::of_subject(subject) {
                crate::sys::io::loader::open_asset(credit, load);
            }
        }
    });
    // Collected first: stopping a read settles it, which must not happen inside
    // the request scheduler's own walk.
    let mut stops = List::<u64, { crate::sys::io::param::MAX_CREDITS as usize }>::new();
    take_stops(|key| stops.push(key));
    for key in stops {
        if !crate::sys::io::fetch::cancel(key) {
            crate::sys::io::loader::cancel_asset(key);
        }
    }
}
