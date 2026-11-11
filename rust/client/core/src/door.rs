// pxl-core/door.rs

//! The core's side of the door. Every turn enters through `run`
//! (xpute_core::entry), which applies the commands waiting for the core and
//! sends the turn's events. The front and its header belong to xpute: the
//! runtime lays the header where the core's manifest places it (manifest.json,
//! xpute_core::manifest). xpute also defines what the runtime hands the core at
//! boot (`BootInfo`).

mod apply;
/// The commands and the snapshot the core and the UI agree on, generated
/// from ui.json. The enums the domain reads are generated apart
/// (domain/cmd.spec.rs), and this module names them from there. What every
/// core shares with every runtime is in `xpute_core::abi`.
#[path = "door/cmd.spec.rs"]
pub mod cmd;
pub mod layout;
mod life;
#[cfg(not(target_arch = "wasm32"))]
mod touch;

use crate::domain::control::sensor;
use crate::domain::scene::character::surface::character_surface_close;
use crate::domain::view::demand;
use crate::door::cmd::{Major, Sensor, SensorStart, SensorStop, UiSnapshot};
use crate::perf::record::{perf_record, perf_record_turn};
use crate::perf::spec::Wake;
use crate::perf::task::TaskRecord;
use crate::sys::boot;
use crate::sys::io::dispatch;
use crate::sys::io::file;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::io::runtime;
use crate::sys::io::text;
#[cfg(not(target_arch = "wasm32"))]
use crate::sys::io::text_device;
use crate::sys::sched::executor;
use crate::sys::sched::{self, request_world_tick, wake_at};
use crate::sys::section;
use crate::turn;
use crate::ui;
use crate::ui::SnapshotMemo;
use pxl_render::gpu::api;
use pxl_render::gpu::surface;
use pxl_render::renderer;
use xpute_core::clock::now;
use xpute_core::entry::BootInfo;
use xpute_core::global::Global;
use xpute_core::io::door::{door, Event};
use xpute_core::mem::off_of_slice;
use xpute_core::mem::section::PAGE_BYTES;
use xpute_core::stop;
use xpute_gpu::event::{self as gpu_event, Event as GpuEvent};

struct State {
    /// The UI snapshot and view last posted. Each is posted again only when
    /// it changes.
    last_snapshot: SnapshotMemo,
    /// Set when a UI command was applied this turn, so the turn posts to the
    /// UI as it ends.
    snapshot_due: bool,
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        last_snapshot: Default::default(),
        snapshot_due: false,
    })
}

/// Closes a surface, and the character drawn on it closes with it.
fn close_surface(handle: u32) {
    if let Some(closed) = surface::surface_close(handle) {
        renderer::on_surface_closed(closed);
    }
    character_surface_close(handle);
}

/// Called when the GPU answers, the first time or again after a loss. The
/// core then asks for the device's block and makes the renderer on it once
/// the block is in (pxl-render gpu/api.rs). A later loss is reported through
/// the block and read at the start of a turn.
fn gpu_ready() {
    api::on_device_answered();
    request_world_tick();
}

/// Called when a UI has booted. On a device the core outlives a JavaScript
/// reload, and the old UI will never answer its reads, surfaces, sensor
/// watches or text requests. A read left open would hold its credit forever,
/// so all of them are dropped here.
fn on_runtime_attached(s: &mut State) {
    request::reset();
    let open: Vec<u32> = surface::surfaces().map(|surface| surface.handle).collect();
    for handle in open {
        close_surface(handle);
    }
    sensor::sensors_close();
    text::text_forget();
    apply::input::forget_pointers();
    s.last_snapshot = Default::default();
}

/// Boots the core on its first turn, after its form has checked the front the
/// runtime laid against the manifest and opened it (`xpute_core::core!`). The
/// stop goes in first, and the rest of memory comes last.
fn boot(b: BootInfo) {
    stop::install(runtime::abort, b.stop_hook);
    #[cfg(not(target_arch = "wasm32"))]
    {
        text_device::install();
    }
    boot::set(b);
    layout::install(&b);
    // A layer asks for no more tiles at once than its tile cache has slots.
    request::open(|c| demand::of_stream(c).map_or(0, |l| l.slot_cnt));
    turn::boot();
}

/// The core's entry (xpute_core::entry `Entry`). A turn applies the waiting
/// events, polls the tasks against the deadline, and returns when to
/// wake next, in milliseconds: 0 for the next frame, NO_WAKE for never. The
/// first turn boots from what the runtime's form hands it (`boot_info`),
/// which includes the stop hook and the GPU runtime's admission.
pub fn run(deadline_ms: f64, boot_info: fn() -> BootInfo) -> f64 {
    use crate::turn::{take_new_frame, world_frame_waiting};
    if section::fixed().is_none() {
        boot(boot_info());
    }
    let s = state();
    let start = now();
    sched::start(deadline_ms);
    // A device loss, which the runtime wrote into the device's block, is read
    // before the commands, because a GPU_READY among them asks for that
    // block anew.
    if api::take_device_lost() {
        renderer::on_gpu_lost();
        loader::on_images_lost();
    }
    // The device finished an image or a pipeline since the last turn. Its
    // fence's store woke this turn, and a frame must take it in.
    if renderer::complete() {
        request_world_tick();
    }
    door().receive(|event| match event {
        Event::Life(l) => life::apply(s, l),
        Event::Turn(t) => apply::turn::turn(t),
        Event::Frame(f) => apply::turn::grant(f),
        Event::Changed(c) => apply::fs::changed(c),
        Event::Completion(c) => {
            file::on_completion(&c);
            Ok(Some(0))
        }
        Event::Program(number, pkt) => match gpu_event::decode(Major::GPU as u32, number, pkt) {
            Ok(Some(GpuEvent::Ready)) => {
                gpu_ready();
                Ok(Some(0))
            }
            Ok(Some(GpuEvent::Surface(c))) => apply::on_surface(c),
            Ok(None) => cmd::ui::apply(s, number, &pkt),
            Err(e) => Err(e),
        },
        _ => Ok(None),
    });
    let drained = now();
    if deadline_ms > 0.0 || world_frame_waiting() {
        executor::poll();
    }
    let polled = now();
    // The UI is posted to at most once a turn, and only when a frame was
    // drawn or a UI command was applied.
    let due = core::mem::take(&mut s.snapshot_due);
    if take_new_frame() || due {
        post_ui(s);
    }
    // The request scheduler's events, in order. A start or a stop goes to
    // the stream's reader, and an end becomes a row in the profiler.
    while let Some(event) = request::take_event() {
        match event {
            request::Event::Start(key, stream, id) => dispatch::start(key, stream, id),
            request::Event::Stop(key) => dispatch::stop(key),
            request::Event::End(a, at, exit, phases) => {
                perf_record(|| TaskRecord::of(&a, at, exit, phases));
            }
        }
    }
    api::gpu_publish();
    if let Some(at) = request::next_pump_at(now()) {
        wake_at(at);
    }
    let wake = sched::wake_ms();
    let kind = match wake {
        0.0 => Wake::next,
        w if w < 0.0 => Wake::none,
        _ => Wake::timer,
    };
    let end = now();
    perf_record_turn(end - start, polled - drained, f64::from(kind as u8));
    wake
}

/// The buffer the UI reads the snapshot from. It grows to the largest
/// snapshot and keeps that size, so a settled turn allocates nothing. The
/// runtime takes what the snapshot's event names as the event is sent
/// (xpute_core io/door.rs R1), so the buffer is the core's again at once.
static SNAPSHOT: Global<Vec<u8>> = Global::new();

fn post_ui(s: &mut State) {
    use crate::domain::control::sensor::device_orientation::{
        take_orient_command, OrientationCommand,
    };
    use crate::domain::control::sensor::geolocation::{
        take_geolocation_command, GeolocationCommand,
    };

    let start = |sensor: Sensor| cmd::ui::sensor_start(&SensorStart { sensor });
    let stop = |sensor: Sensor| cmd::ui::sensor_stop(&SensorStop { sensor });
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
    if let Some(view) = s.last_snapshot.take_view_changed() {
        cmd::ui::ui_view(&view);
    }
    if let Some(shown) = s.last_snapshot.take_changed() {
        let buf = SNAPSHOT.get(Vec::new);
        let len = loop {
            if let Some(len) = ui::snapshot(&shown, buf) {
                break len;
            }
            let room = (buf.len() * 2).max(PAGE_BYTES as usize);
            buf.resize(room, 0);
        };
        cmd::ui::ui_snapshot(&UiSnapshot {
            at: off_of_slice(&raw const buf[..]) as u32,
            len,
        });
    }
}
