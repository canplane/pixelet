// pxl-kernel/door.rs

//! The kernel's side of the doorbell, which a bracket (pxl-wasm, pxl-native)
//! hands each turn to: the toc, commands applied, events raised. The
//! rings are xpute's (`xpute_guest::ipc::door`), and so is the entry the
//! kernel implements (`run`, xpute_guest::entry) and what its host's form
//! hands it (`BootInfo`).

mod apply;
mod executor;
pub mod layout;
/// The ports at the kernel's far side, generated from port.json.
#[path = "door/port.spec.rs"]
pub(crate) mod port;
/// The toc's keys, generated from toc.json.
#[path = "door/toc.spec.rs"]
pub(crate) mod toc;

use crate::domain::control::cmd::{
    Sensor, SensorStartEvent, SensorStopEvent, ShellSnapshotEvent, TextLayoutEvent, TextRasterEvent,
};
use crate::domain::control::sensor;
use crate::domain::scene::character::preview;
use crate::domain::view::demand;
use crate::door::toc::key::GPU_STREAM;
use crate::perf::record::perf_record_turn;
use crate::perf::spec::Wake;
use crate::shell;
use crate::shell::SnapshotMemo;
use crate::sys::boot;
use crate::sys::io::dispatch;
use crate::sys::io::file;
use crate::sys::io::host;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::io::text;
use crate::sys::io::text::TextCall;
use crate::sys::mem::section;
use crate::sys::mem::spec;
use crate::sys::sched::edge;
use crate::turn;
use pxl_render::gpu::api;
use pxl_render::gpu::renderer;
use pxl_render::gpu::surface;
use xpute_gpu::admit::Admit;
use xpute_guest::clock::now;
use xpute_guest::entry::BootInfo;
use xpute_guest::global::Global;
use xpute_guest::ipc::door::{Door, DoorStat, Submission};
use xpute_guest::ipc::toc::{spec::key, toc_words, write_toc, Entry};
use xpute_guest::mem::section::PAGE_BYTES;
use xpute_guest::mem::{self, off_of_slice};
use xpute_guest::sched::task;
use xpute_guest::stop::{self, PanicHook};
use xpute_kit::status::errno::Errno;

struct State {
    /// The UI snapshot last posted; one is posted only where it moved.
    last_snapshot: SnapshotMemo,
    /// A UI command was applied this turn, so its falling edge posts to the shell.
    asked: bool,
}

static DOOR: Global<Door> = Global::new();

pub(crate) fn door() -> &'static Door {
    DOOR.get(|| {
        // SAFETY: once, the door's first use; the ranges are the layout's alone.
        let (sq, cq) = unsafe { layout::rings() };
        Door::new(sq, cq, EVENTS)
    })
}

/// The kernel's text call, made of the shell's port as TEXT_LAYOUT or
/// TEXT_RASTER and answered before it returns (sys/io/host.rs `Hosted`).
pub(crate) fn text_call(call: &TextCall<'_>) -> Result<u32, Errno> {
    match *call {
        TextCall::Layout {
            weight,
            px,
            max_width,
            at,
            lines,
            text,
        } => door().call(
            port::SHELL,
            &TextLayoutEvent {
                weight,
                px,
                max_width,
                at,
                lines,
                text_at: off_of_slice(text.as_bytes()) as u32,
                text_len: text.len() as u32,
            },
        ),
        TextCall::Raster {
            weight,
            px,
            color,
            at,
            width,
            height,
            x,
            text,
        } => door().call(
            port::SHELL,
            &TextRasterEvent {
                weight,
                px,
                color,
                at,
                width,
                height,
                x,
                text_at: off_of_slice(text.as_bytes()) as u32,
                text_len: text.len() as u32,
            },
        ),
    }
}

/// The rings' peaks since the last take, the profiler's (perf/report.rs
/// `perf_frame`).
pub(crate) fn take_stat() -> DoorStat {
    door().take_stat()
}

/// What a turn posts besides its replies: a sensor's start or stop each and
/// the snapshot, all after its drain and once (`post_shell` at the falling
/// edge); the files' hostcalls at their most, whether the drain or the rest
/// of the turn posts them (`host::FILE_FRAMES`); and the slot a call is laid
/// in, never published. The door's reserve past this is a command's reply.
const EVENTS: u32 = Sensor::ALL.len() as u32 + 1 + host::FILE_FRAMES + 1;

// The host reads the completion ring dry before it rings, so a ring with
// room for a reply to every submission and the door's reserve past the last
// (`Door::new`, EVENTS + 1) never stops a drain partway (mem.json
// CQ_CAP).
const _: () = assert!(
    spec::SQ_CAP + EVENTS < spec::CQ_CAP,
    "a full submission ring's replies and the turn's posts do not fit the completion ring"
);

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        last_snapshot: Default::default(),
        asked: false,
    })
}

/// xpute's two rings, and the GPU stream.
const TOC_ENTRIES: usize = 3;

/// A case lives on its surface, so it goes with it.
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
    sensor::sensors_close();
    text::text_forget();
    s.last_snapshot = Default::default();
}

/// First turn: lays out memory and writes the toc at TOC_AT.
fn boot(b: BootInfo, panic_hook: Option<PanicHook>, admit: Admit) {
    boot::set(b);
    boot::set_admit(admit);
    executor::install();
    stop::install(host::abort, panic_hook);
    let gpu_stream = layout::install(&b);
    // A layer asks for no more tiles than its tile cache's slots.
    request::open(|c| demand::of_stream(c).map_or(0, |l| l.slots));
    turn::boot();
    let entries: [Entry; TOC_ENTRIES] = [
        (key::SQ, door().sq().at() as u32),
        (key::CQ, door().cq().at() as u32),
        (GPU_STREAM, gpu_stream),
    ];
    // SAFETY: the page at TOC_AT, kept clear of the rings and the
    // bracket's data; the toc's alone.
    unsafe {
        write_toc(
            mem::core(),
            spec::TOC_AT as usize,
            toc_words(TOC_ENTRIES),
            &entries,
        );
    };
}

/// The entry (xpute_guest::entry `Entry`, with the bracket's panic hook and
/// its GPU host's admission): a turn applies waiting submissions, polls tasks
/// on the quota, and returns when to wake next, in ms (0 for the next frame,
/// NO_WAKE for none). The first turn boots, on what its host's form hands it
/// (`boot_info`). `panic_hook` is where a bracket sharing its process with the
/// kernel says why, a panic's text not being in the record's numbers, and ends
/// the kernel's thread without the process. None where the host is apart (a page).
pub fn run(
    quota_ms: f64,
    boot_info: fn() -> BootInfo,
    panic_hook: Option<PanicHook>,
    admit: Admit,
) -> f64 {
    use crate::turn::{take_world_drawn, world_frame_waiting};
    if section::map().is_none() {
        boot(boot_info(), panic_hook, admit);
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
    door().receive(|submission| match submission {
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
    let fall = edge::world_fall();
    // What the completion ring had no room for is still in the submission ring.
    let wake = if door().pending() { 0.0 } else { fall };
    let asked = match wake {
        0.0 => Wake::next,
        w if w < 0.0 => Wake::none,
        _ => Wake::timer,
    };
    let fell = now();
    perf_record_turn(fell - rise, polled - drained, f64::from(asked as u8));
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

    let start = |sensor: Sensor| door().send(port::SHELL, &SensorStartEvent { sensor });
    let stop = |sensor: Sensor| door().send(port::SHELL, &SensorStopEvent { sensor });
    if let Some(c) = take_geolocation_command() {
        match c {
            GeolocationCommand::Start => start(Sensor::Geolocation),
            GeolocationCommand::Stop => stop(Sensor::Geolocation),
        }
    }
    if let Some(c) = take_orient_command() {
        match c {
            OrientationCommand::Listen => start(Sensor::Orientation),
            OrientationCommand::Remove => stop(Sensor::Orientation),
        }
    }
    if let Some(shown) = s.last_snapshot.take_moved() {
        let buf = SNAPSHOT.get(Vec::new);
        let len = loop {
            if let Some(len) = shell::ui_snapshot(&shown, buf) {
                break len;
            }
            let room = (buf.len() * 2).max(PAGE_BYTES as usize);
            buf.resize(room, 0);
        };
        door().send(
            port::SHELL,
            &ShellSnapshotEvent {
                at: off_of_slice(&raw const buf[..]) as u32,
                len,
            },
        );
    }
}
