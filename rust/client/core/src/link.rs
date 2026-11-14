// pxl-core/link.rs

//! The core's side of the link: its boot, and the start and end of every
//! entry, which xpute's link runs (lib.rs `link!`). The boot mounts what the
//! core serves and starts its tasks; the start begins the entry's budget;
//! the end runs the core's tasks until none is ready and tells the UI what
//! changed, and the overlay whole to a UI that has just started, its tile
//! source at once (`tell_all`).

pub mod bound;
mod export;
mod gpu;
pub mod layout;
mod life;

use crate::abi;
use crate::abi::types::Sensor;
use crate::domain::layer::object::repository;
use crate::domain::scene::character::surface::character_surface_close;
use crate::domain::setting::{on_settings_set_aside, settings_open};
use crate::domain::view::demand;
use crate::frame;
use crate::perf::record::perf_record;
use crate::perf::task::TaskRecord;
use crate::sys::io::dispatch;
use crate::sys::io::file;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::io::source;
use crate::sys::sched::executor;
use crate::sys::sched::{self, invalidate, invalidate_at};
use crate::ui;
use crate::ui::SnapshotMemo;
use pxl_global::Global;
use pxl_render::gpu::api;
use pxl_render::gpu::surface;
use pxl_render::renderer;
use pxl_render::spec::api::stream;
use xpute_guest::clock::now;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

struct State {
    /// The UI snapshot and view last posted. Each is posted again only when
    /// it changes, or when a UI starts (`tell_all`).
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
    if let Some((slot, closed)) = surface::surface_close(handle) {
        renderer::on_surface_closed(slot, closed);
    }
    character_surface_close(handle);
}

/// Boots the core in its first entry (xpute-guest link `boot`).
pub fn boot() {
    layout::install();
    export::mount();
    // The renderer's stream is laid at the size api.json5 gives it, and
    // then the device it runs on is taken.
    api::install(stream::BYTES);
    gpu::boot();
    sched::boot();
    // A layer asks for no more tiles at once than its tile cache has slots.
    request::open(|c| demand::of_stream(c).map_or(0, |l| l.slot_cnt));
    // The settings' slot is opened here and read by the frames that follow.
    // What waits on the settings, the profiler and the snapshot, starts once
    // they are in (frame.rs). The objects are read a shard at a time as the
    // view asks for them (domain/layer/object/repository.rs).
    settings_open();
    // The UI must hold the core's state from its first render on.
    invalidate();
    frame::boot();
}

/// Starts an entry (xpute-guest link `boot`, `recv`): its budget.
pub fn enter() {
    sched::begin();
}

/// Ends an entry, as xpute's link runs it once the due answers have run, and
/// again until it leaves no work (xpute-guest link `leave`). Each run reads
/// what the GPU did, polls the core's tasks, and answers that work is left
/// while one is ready. With none ready, it hands the request scheduler's
/// events on, arms the next pump, flushes the GPU's stream and posts to the
/// UI what changed, and answers whether any of that readied a task.
pub fn end() -> bool {
    // A device loss, which the host wrote into the device's block, is read
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
    // A refused file set aside goes back to its owner: the settings write
    // theirs from their next keep on, and the objects' file is read again now.
    while let Some(r) = file::take_set_aside() {
        match r {
            file::Refused::Storage => on_settings_set_aside(),
            file::Refused::Object(id) => _ = repository::on_set_aside(id),
        }
    }
    if let Some(at) = request::next_pump_at(now()) {
        invalidate_at(at);
    }
    api::gpu_flush();
    post_ui(state());
    executor::pending()
}

/// Tells a UI that has just started where everything stands: the tile source
/// at once (sys/io/source.rs `tell`), and the snapshot and the view whole at
/// this entry's end, as nothing was posted to it. The runtime tells the core
/// each UI that attaches (xpute's `life.attached`), the first and one a
/// reload makes on the same core alike; its sensors start with its world's
/// surface (`sensors_open`).
fn tell_all() {
    source::tell();
    state().last_snapshot = SnapshotMemo::default();
}

/// Tells the other side what changed: the sensors to watch or not, and what
/// is drawn over the world, the view and the snapshot. A call to the UI answers at once, its args copied across.
fn post_ui(s: &mut State) {
    use crate::domain::control::sensor::device_orientation::{take_orient_cmd, OrientationCommand};
    use crate::domain::control::sensor::geolocation::{take_geolocation_cmd, GeolocationCommand};

    let start = |sensor: Sensor| abi::watch::start(sensor).or_bug(Errno::EPROTO);
    let stop = |sensor: Sensor| abi::watch::stop(sensor).or_bug(Errno::EPROTO);
    if let Some(c) = take_geolocation_cmd() {
        match c {
            GeolocationCommand::Start => start(Sensor::Geolocation),
            GeolocationCommand::Stop => stop(Sensor::Geolocation),
        }
    }
    if let Some(c) = take_orient_cmd() {
        match c {
            OrientationCommand::Start => start(Sensor::Orient),
            OrientationCommand::Stop => stop(Sensor::Orient),
        }
    }
    if let Some((azimuth, m_per_px)) = s.last_snapshot.take_view_changed() {
        abi::overlay::view(azimuth, m_per_px).or_bug(Errno::EPROTO);
    }
    if let Some(shown) = s.last_snapshot.take_changed() {
        abi::overlay::show(&ui::snapshot(&shown)).or_bug(Errno::EPROTO);
    }
}
