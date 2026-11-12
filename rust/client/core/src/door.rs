// pxl-core/door.rs

//! The core's side of the link: its boot, and the start and end of every
//! entry, which xpute's link runs (lib.rs `link!`). The boot mounts what the
//! core serves and opens what it calls; the start begins the entry's budget;
//! the end runs the core's tasks until none is ready and tells the UI what
//! changed.

mod apply;
pub mod layout;
mod life;
#[cfg(not(target_arch = "wasm32"))]
mod touch;

use crate::abi;
use crate::abi::types::Sensor;
use crate::domain::scene::character::surface::character_surface_close;
use crate::domain::setting::store::settings_open;
use crate::domain::view::demand;
use crate::perf::record::perf_record;
use crate::perf::task::TaskRecord;
use crate::sys::io::dispatch;
use crate::sys::io::loader;
use crate::sys::io::request;
#[cfg(not(target_arch = "wasm32"))]
use crate::sys::io::text_device;
use crate::sys::sched::executor;
use crate::sys::sched::{self, invalidate, invalidate_at};
use crate::turn;
use crate::ui;
use crate::ui::SnapshotMemo;
use pxl_render::gpu::api;
use pxl_render::gpu::surface;
use pxl_render::renderer;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

struct State {
    /// The UI snapshot and view last posted. Each is posted again only when
    /// it changes.
    last_snapshot: SnapshotMemo,
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        last_snapshot: Default::default(),
    })
}

/// Closes a surface, and the character drawn on it closes with it.
fn close_surface(handle: u32) {
    if let Some(closed) = surface::surface_close(handle) {
        renderer::on_surface_closed(closed);
    }
    character_surface_close(handle);
}

/// Boots the core in its first entry (xpute-core link `init`).
pub fn boot() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        text_device::install();
    }
    apply::mount();
    // The runtime's singletons the core calls, each opened once.
    let opened = [
        xpute_core::abi::timer::open(),
        xpute_core::abi::clock::open(),
        xpute_gpu::abi::vsync::open(),
        abi::ui::open(),
        abi::text::open(),
    ];
    opened.into_iter().collect::<Result<(), Errno>>().or_bug(Errno::ENOENT);
    // A layer asks for no more tiles at once than its tile cache has slots.
    request::open(|c| demand::of_stream(c).map_or(0, |l| l.slot_cnt));
    // The settings' slot is opened here and read by the turns that follow.
    // What waits on the settings, the profiler and the snapshot, starts once
    // they are in (turn.rs). The objects are read a shard at a time as the
    // view asks for them (domain/layer/object/repository.rs).
    settings_open();
    // The UI must hold the core's state from its first render on.
    invalidate();
    turn::boot();
}

/// Starts an entry (xpute-core link `init`): its budget.
pub fn enter() {
    sched::begin();
}

/// Ends an entry, as xpute's link runs it once the due answers have run, and
/// again until it leaves no work (xpute-core link `init`). Each run reads
/// what the GPU did, polls the core's tasks, and answers that work is left
/// while one is ready. With none ready, it hands the request scheduler's
/// events on, arms the next pump, publishes the GPU's stream and posts to the
/// UI what changed, and answers whether any of that readied a task.
pub fn end() -> bool {
    // A device loss, which the runtime wrote into the device's block, is read
    // before the tasks run.
    if api::take_device_lost() {
        renderer::on_gpu_lost();
        loader::on_images_lost();
    }
    // The device finished an image or a pipeline, and a frame must take it
    // in.
    if renderer::complete() {
        invalidate();
    }
    executor::poll();
    if executor::pending() {
        return true;
    }
    // The request scheduler's events, in order. A start or a stop goes to
    // the stream's reader, and an end becomes a row in the profiler.
    while let Some(event) = request::take_event() {
        match event {
            request::Event::Start(key, stream, id) => dispatch::start(key, stream, id),
            request::Event::Stop(key) => dispatch::stop(key),
            request::Event::End(a, at, exit, phases) => {
                perf_record(|name_at| TaskRecord::of(&a, at, exit, phases, name_at));
            }
        }
    }
    if let Some(at) = request::next_pump_at(now()) {
        invalidate_at(at);
    }
    api::gpu_publish();
    post_ui(state());
    executor::pending()
}

/// Tells the UI what changed: the sensors to start or stop, the view, and
/// the snapshot. A call to the UI answers at once, its args copied across.
fn post_ui(s: &mut State) {
    use crate::domain::control::sensor::device_orientation::{take_orient_cmd, OrientationCommand};
    use crate::domain::control::sensor::geolocation::{take_geolocation_cmd, GeolocationCommand};

    let start = |sensor: Sensor| abi::ui::sensor_start(sensor).or_bug(Errno::EPROTO);
    let stop = |sensor: Sensor| abi::ui::sensor_stop(sensor).or_bug(Errno::EPROTO);
    if let Some(c) = take_geolocation_cmd() {
        match c {
            GeolocationCommand::Start => start(Sensor::Geolocation),
            GeolocationCommand::Stop => stop(Sensor::Geolocation),
        }
    }
    if let Some(c) = take_orient_cmd() {
        match c {
            OrientationCommand::Listen => start(Sensor::Orientation),
            OrientationCommand::Remove => stop(Sensor::Orientation),
        }
    }
    if let Some((azimuth, m_per_px)) = s.last_snapshot.take_view_changed() {
        abi::ui::view(azimuth, m_per_px).or_bug(Errno::EPROTO);
    }
    if let Some(shown) = s.last_snapshot.take_changed() {
        abi::ui::show(&ui::snapshot(&shown)).or_bug(Errno::EPROTO);
    }
}
