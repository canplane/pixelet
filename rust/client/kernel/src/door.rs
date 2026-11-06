// pxl-kernel/door.rs

//! The kernel's side of the doorbell, its entry (`run`, xpute_guest::entry),
//! which its machine's guest form (`xpute_guest::guest!`, lib.rs) hands each
//! turn to: commands applied, events raised. The rings, the front's header
//! and the front they lie in are xpute's, laid by the host as the kernel's
//! machine places them (machine.json, xpute_guest::machine), and so is what
//! its host's form hands it (`BootInfo`).

mod apply;
mod executor;
pub mod layout;
mod life;

use crate::domain::control::cmd::{
    self, Sensor, SensorStartEvent, SensorStopEvent, ShellSnapshotEvent, TextLayoutEvent,
    TextRasterEvent,
};
use crate::domain::control::sensor;
use crate::domain::scene::character::preview;
use crate::domain::view::demand;
use crate::machine::MACHINE;
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
use crate::sys::sched::edge;
use crate::turn;
use pxl_render::gpu::api;
use pxl_render::gpu::renderer;
use pxl_render::gpu::surface;
use xpute_guest::clock::now;
use xpute_guest::entry::BootInfo;
use xpute_guest::global::Global;
use xpute_guest::ipc::door::{DoorStat, Pos, Submission};
use xpute_guest::machine::{self, door};
use xpute_guest::mem::off_of_slice;
use xpute_guest::mem::section::PAGE_BYTES;
use xpute_guest::sched::task;
use xpute_guest::stop;
use xpute_kit::status::errno::Errno;

struct State {
    /// The UI snapshot and view last posted; each is posted only where it moved.
    last_snapshot: SnapshotMemo,
    /// Where the last snapshot's event was posted: `SNAPSHOT` is the host's
    /// until its head has passed it (xpute_guest ipc/door.rs R3).
    snapshot_pos: Option<Pos>,
    /// A snapshot was put off while the host still held the last: the next
    /// turn comes at once and posts it.
    snapshot_pending: bool,
    /// A UI command was applied this turn, so its falling edge posts to the shell.
    snapshot_due: bool,
}

/// The kernel's text call, made of the shell's protocol as TEXT_LAYOUT or
/// TEXT_RASTER and answered before it returns (sys/io/host.rs `Host`).
pub(crate) fn text_call(call: &TextCall<'_>) -> Result<u32, Errno> {
    match *call {
        TextCall::Layout {
            weight,
            px,
            max_width,
            at,
            lines,
            text,
        } => cmd::shell::text_layout(&TextLayoutEvent {
            weight,
            px,
            max_width,
            at,
            lines,
            text_at: off_of_slice(text.as_bytes()) as u32,
            text_len: text.len() as u32,
        }),
        TextCall::Raster {
            weight,
            px,
            color,
            at,
            width,
            height,
            x,
            text,
        } => cmd::shell::text_raster(&TextRasterEvent {
            weight,
            px,
            color,
            at,
            width,
            height,
            x,
            text_at: off_of_slice(text.as_bytes()) as u32,
            text_len: text.len() as u32,
        }),
    }
}

/// The rings' peaks since the last take, the profiler's (perf/report.rs
/// `perf_frame`).
pub(crate) fn take_stat() -> DoorStat {
    door().take_stat()
}

/// What a turn posts besides its replies: a sensor's start or stop each, the
/// snapshot and the view, all after its drain and once (`post_shell` at the
/// falling edge); the files' hostcalls at their most, whether the drain or the
/// rest of the turn posts them (`host::FILE_FRAMES`); and the slot a call is
/// laid in, never published. The machine declares it (machine.json `turn_posts`).
const EVENTS: u32 = Sensor::ALL.len() as u32 + 2 + host::FILE_FRAMES + 1;

const _: () = assert!(
    MACHINE.turn_posts == EVENTS,
    "machine.json's turn_posts is not what a turn posts"
);

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        last_snapshot: Default::default(),
        snapshot_pos: None,
        snapshot_pending: false,
        snapshot_due: false,
    })
}

/// A case lives on its surface, so it goes with it.
fn close_surface(handle: u32) {
    if let Some(closed) = surface::surface_close(handle) {
        renderer::on_surface_closed(closed);
    }
    preview::preview_close(handle);
}

/// The GPU answered, the first time or again after a loss: the kernel asks
/// its block and makes the renderer on it once the block is in (pxl-render
/// gpu/api.rs). Its loss is the block's to say, read at the turn's edge.
fn gpu_ready() {
    api::on_device_answered();
    edge::request_world_tick();
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

/// First turn: the stop installed, then the front the host laid held to the
/// machine and opened where the form has not (`xpute_guest::guest!` has, on a
/// page), then the rest of memory.
fn boot(b: BootInfo) {
    stop::install(host::abort, b.stop_hook);
    if !machine::booted() {
        machine::boot(&MACHINE);
    }
    boot::set(b);
    executor::install();
    layout::install(&b);
    // A layer asks for no more tiles than its tile cache's slots.
    request::open(|c| demand::of_stream(c).map_or(0, |l| l.slots));
    turn::boot();
}

/// The entry (xpute_guest::entry `Entry`): a turn applies waiting
/// submissions, polls tasks on the quota, and returns when to wake next, in
/// ms (0 for the next frame, NO_WAKE for none). The first turn boots, on what
/// its host's form hands it (`boot_info`), its stop hook and its GPU host's
/// admission among it.
pub fn run(quota_ms: f64, boot_info: fn() -> BootInfo) -> f64 {
    use crate::turn::{take_world_drawn, world_frame_waiting};
    if section::map().is_none() {
        boot(boot_info());
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
    // An image or a pipeline the device made since: its fence's store woke
    // this turn, and a frame takes it.
    if renderer::landed() {
        edge::request_world_tick();
    }
    door().receive(|submission| match submission {
        Submission::Life(l) => life::apply(s, l),
        Submission::GpuReady => {
            gpu_ready();
            Ok(Some(0))
        }
        Submission::Opened(opened) => {
            file::on_opened(&opened);
            Ok(Some(0))
        }
        Submission::Closed(_) => Ok(Some(0)),
        Submission::Program(number, pkt) => cmd::shell::apply(s, number, &pkt),
    });
    let drained = now();
    if quota_ms > 0.0 || world_frame_waiting() {
        task::poll();
    }
    let polled = now();
    // Once a turn, where a frame was drawn, a UI command applied or a
    // snapshot put off.
    let due = core::mem::take(&mut s.snapshot_due);
    if take_world_drawn() || due || core::mem::take(&mut s.snapshot_pending) {
        post_shell(s);
        dispatch::run();
    }
    api::gpu_publish();
    let fall = edge::world_fall();
    // What the completion ring had no room for is still in the submission
    // ring, and a snapshot put off waits for the next turn.
    let wake = if door().pending() || s.snapshot_pending {
        0.0
    } else {
        fall
    };
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
/// It is rewritten only once the host's head has passed the event that named
/// it (`State::snapshot_pos`); until then a snapshot that moved is put off.
static SNAPSHOT: Global<Vec<u8>> = Global::new();

fn post_shell(s: &mut State) {
    use crate::domain::control::sensor::device_orientation::{
        take_orient_command, OrientationCommand,
    };
    use crate::domain::control::sensor::geolocation::{
        take_geolocation_command, GeolocationCommand,
    };

    let start = |sensor: Sensor| cmd::shell::sensor_start(&SensorStartEvent { sensor });
    let stop = |sensor: Sensor| cmd::shell::sensor_stop(&SensorStopEvent { sensor });
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
    if let Some(view) = s.last_snapshot.take_view_moved() {
        cmd::shell::shell_view(&view);
    }
    if s.snapshot_pos.is_some_and(|p| !door().is_consumed(p)) {
        // The memo is left as it was, so what moved is posted once the host lets go.
        s.snapshot_pending = true;
    } else if let Some(shown) = s.last_snapshot.take_moved() {
        let buf = SNAPSHOT.get(Vec::new);
        let len = loop {
            if let Some(len) = shell::ui_snapshot(&shown, buf) {
                break len;
            }
            let room = (buf.len() * 2).max(PAGE_BYTES as usize);
            buf.resize(room, 0);
        };
        cmd::shell::shell_snapshot(&ShellSnapshotEvent {
            at: off_of_slice(&raw const buf[..]) as u32,
            len,
        });
        s.snapshot_pos = Some(door().last_pos());
    }
}
