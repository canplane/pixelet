// pxl-kernel/door.rs

//! The kernel's side of the doorbell, which a bracket (pxl-wasm, pxl-native)
//! hands each turn to: the directory, commands applied, events raised. The
//! rings are xpute's (`xpute_guest::ipc::door`), and so is the entry the
//! kernel implements (`run`, xpute_guest::entry) and what its host's form
//! hands it (`BootInfo`).

mod apply;
/// The directory's keys, generated from directory.json.
#[path = "door/directory.spec.rs"]
pub mod directory;
mod executor;
pub mod layout;

use crate::domain::control::cmd::{Command, Sensor, SensorStartEvent, WorldSnapshotEvent};
use crate::domain::scene::character::preview;
use crate::domain::view::demand;
use crate::door::directory::key::GPU_STREAM;
use crate::profiler::record::profiler_record_turn;
use crate::profiler::spec::Wake;
use crate::shell;
use crate::shell::Sent;
use crate::sys::boot;
use crate::sys::io::dispatch;
use crate::sys::io::file;
use crate::sys::io::host;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::io::text;
use crate::sys::mem::section;
use crate::sys::mem::spec;
use crate::sys::sched::edge;
use crate::sys::stop;
use crate::sys::stop::LastWords;
use crate::turn;
use pxl_render::gpu::api;
use pxl_render::gpu::renderer;
use pxl_render::gpu::surface;
use xpute_gpu::admit::Admit;
use xpute_guest::clock::now;
use xpute_guest::entry::BootInfo;
use xpute_guest::global::Global;
use xpute_guest::ipc::directory::{directory_words, spec::key, write_directory, Entry};
use xpute_guest::ipc::door::{Door, DoorStat, Submission};
use xpute_guest::mem::section::PAGE_BYTES;
use xpute_guest::mem::{self, off_of_slice};
use xpute_guest::sched::task;
use xpute_kit::wire::xtp::PacketWriter;

struct State {
    /// The UI snapshot last posted; one is posted only where it moved.
    sent: Sent,
    /// A UI command was applied this turn, so its falling edge posts to the shell.
    asked: bool,
}

static DOOR: Global<Door> = Global::new();

fn door() -> &'static Door {
    DOOR.get(|| {
        // SAFETY: once, the door's first use; the ranges are the layout's alone.
        let (sq, cq) = unsafe { layout::rings() };
        Door::new(sq, cq, EVENTS)
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
const EVENTS: u32 = Sensor::ALL.len() as u32 + 1;

// The shell reads the completion ring dry before it rings, so a ring with
// room for a reply to every submission and the door's reserve past the last
// (`Door::new`, EVENTS + 1) never stops a drain partway (mem.json
// CQ_CAP).
const _: () = assert!(
    spec::SQ_CAP + EVENTS < spec::CQ_CAP,
    "a full submission ring's replies and the turn's events do not fit the completion ring"
);

fn raise(cmd: Command, cnt: u32, f: impl Fn(&mut PacketWriter)) {
    door().raise(cmd as u32, cnt, f);
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        sent: Default::default(),
        asked: false,
    })
}

/// xpute's two rings and the host's stream, and the GPU stream.
const DIRECTORY_ENTRIES: usize = 4;

/// A case lives on its surface, so it goes with it: a preview never closed
/// held its actor for good.
fn close_surface(handle: u32) {
    if let Some(closed) = surface::surface_close(handle) {
        renderer::on_surface_closed(closed);
    }
    preview::preview_close(handle);
}

/// A shell has booted. On a device the kernel outlives a JS reload, and the
/// old shell's reads, surfaces, sensor watches and text requests will never
/// be answered; a read left out keeps its credit forever, so all of it is
/// let go here.
fn on_host_attached(s: &mut State) {
    request::reset();
    let open: Vec<u32> = surface::surfaces().map(|surface| surface.handle).collect();
    for handle in open {
        close_surface(handle);
    }
    shell::sensors_close();
    text::text_forget();
    s.sent = Default::default();
}

/// First turn: lays out memory and writes the directory at DIRECTORY_AT.
fn boot(b: BootInfo, last_words: Option<LastWords>, admit: Admit) {
    boot::set(b);
    boot::set_admit(admit);
    executor::install();
    stop::install(last_words);
    let gpu_stream = layout::install(&b);
    // A layer asks for no more tiles than its tile cache's slots.
    request::open(|c| demand::of_stream(c).map_or(0, |l| l.slots));
    turn::boot();
    let entries: [Entry; DIRECTORY_ENTRIES] = [
        (key::SQ, door().sq().at() as u32),
        (key::CQ, door().cq().at() as u32),
        (key::SYS, layout::range(layout::Fixed::Host).off),
        (GPU_STREAM, gpu_stream),
    ];
    // SAFETY: the page at DIRECTORY_AT, kept clear of the rings and the
    // bracket's data; the directory's alone.
    unsafe {
        write_directory(
            mem::core(),
            spec::DIRECTORY_AT as usize,
            directory_words(DIRECTORY_ENTRIES),
            &entries,
        );
    };
}

/// The entry (xpute_guest::entry `Entry`, with the bracket's last words and
/// its GPU host's admission): a turn applies waiting submissions, polls tasks
/// on the quota, and returns when to wake next, in ms (0 for the next frame,
/// NO_WAKE for none). The first turn boots, on what its host's form hands it
/// (`boot_info`). `last_words` is where a bracket whose process dies with the
/// kernel reports why: no host is left to run the record, and a panic's text
/// is not in its numbers. None where the host outlives the kernel (a page).
pub fn run(
    quota_ms: f64,
    boot_info: fn() -> BootInfo,
    last_words: Option<LastWords>,
    admit: Admit,
) -> f64 {
    use crate::turn::{take_world_drawn, world_frame_waiting};
    if section::map().is_none() {
        boot(boot_info(), last_words, admit);
    }
    let s = state();
    let rise = now();
    host::rise();
    edge::world_edge().rise(quota_ms);
    // The device's loss, as its host wrote it into the device's block: before
    // the commands, as a device answering again among them asks that block anew.
    if api::take_device_lost() {
        renderer::on_gpu_lost();
        loader::on_images_lost();
    }
    door().drain(|submission| match submission {
        Submission::Opened(opened) => {
            file::on_opened(&opened);
            Ok(Some(0))
        }
        Submission::Program(number, pkt) => apply::apply(s, number, &pkt),
    });
    let drained = now();
    if quota_ms > 0.0 || world_frame_waiting() {
        task::poll();
    }
    let polled = now();
    // Once a turn, where a frame was drawn or a UI command applied: a second
    // snapshot would point into the buffer the first one's event still names.
    let asked = core::mem::take(&mut s.asked);
    if take_world_drawn() || asked {
        post_shell(s);
        dispatch::run();
    }
    api::gpu_publish();
    host::fall();
    let fall = edge::world_fall();
    // What the completion ring had no room for is still in the submission ring.
    let wake = if door().pending() { 0.0 } else { fall };
    let asked = match wake {
        0.0 => Wake::next,
        w if w < 0.0 => Wake::none,
        _ => Wake::timer,
    };
    let fell = now();
    profiler_record_turn(
        fell - rise,
        polled - drained,
        f64::from(asked as u8),
        wake.max(0.0),
    );
    wake
}

/// Grows to the largest snapshot and stays, so a settled turn allocates nothing.
/// One is posted a turn, and the shell reads it on the event, before the next
/// turn can rewrite it.
static SNAPSHOT: Global<Vec<u8>> = Global::new();

fn post_shell(s: &mut State) {
    use crate::domain::control::sensor::device_orientation::{
        take_orient_command, OrientationCommand,
    };
    use crate::domain::control::sensor::geolocation::{
        take_geolocation_command, GeolocationCommand,
    };

    let sensor = |cmd: Command, sensor: Sensor| {
        raise(cmd, SensorStartEvent::FIELDS, |w| {
            SensorStartEvent { sensor }.write(w);
        });
    };
    if let Some(c) = take_geolocation_command() {
        match c {
            GeolocationCommand::Start => sensor(Command::SENSOR_START, Sensor::Geolocation),
            GeolocationCommand::Stop => sensor(Command::SENSOR_STOP, Sensor::Geolocation),
        }
    }
    if let Some(c) = take_orient_command() {
        match c {
            OrientationCommand::Listen => sensor(Command::SENSOR_START, Sensor::Orientation),
            OrientationCommand::Remove => sensor(Command::SENSOR_STOP, Sensor::Orientation),
        }
    }
    if let Some(shown) = s.sent.take_moved() {
        let buf = SNAPSHOT.get(Vec::new);
        let len = loop {
            if let Some(len) = shell::ui_snapshot(&shown, buf) {
                break len;
            }
            let room = (buf.len() * 2).max(PAGE_BYTES as usize);
            buf.resize(room, 0);
        };
        let event = WorldSnapshotEvent {
            at: off_of_slice(&raw const buf[..]) as u32,
            len,
        };
        raise(Command::WORLD_SNAPSHOT, WorldSnapshotEvent::FIELDS, |w| {
            event.write(w);
        });
    }
}
