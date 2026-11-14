// pxl-core/link.rs

//! The core's side of the link: its boot, and the start and end of every
//! entry, which xpute's link runs (lib.rs `link!`). The boot mounts what the
//! core serves and starts its tasks; the start begins the entry's budget;
//! the end runs the core's tasks until none is ready and tells the UI what
//! changed.

pub mod bound;
mod export;
mod gpu;
pub mod layout;
mod life;

use crate::abi;
use crate::abi::types::Sensor;
use crate::domain::layer::object::repository;
use crate::domain::scene::character::surface::character_surface_close;
use crate::domain::setting::store::{on_settings_emptied as settings_emptied, settings_open};
use crate::domain::view::demand;
use crate::frame;
use crate::perf::record::perf_record;
use crate::perf::task::TaskRecord;
use crate::sys::io::dispatch;
use crate::sys::io::file;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::sched::executor;
use crate::sys::sched::{self, invalidate, invalidate_at};
use crate::ui;
use crate::ui::SnapshotMemo;
use pxl_global::Global;
use pxl_render::gpu::api;
use pxl_render::gpu::surface;
use pxl_render::renderer;
use pxl_render::spec::api::stream;
use xpute_guest::abi::stop::stop;
use xpute_guest::clock::now;
use xpute_guest::link::in_guest;
use xpute_kit::status::bug::{self, OrBug};
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
    if let Some((slot, closed)) = surface::surface_close(handle) {
        renderer::on_surface_closed(slot, closed);
    }
    character_surface_close(handle);
}

/// Boots the core in its first entry (xpute-guest link `boot`).
pub fn boot() {
    // From here a bug or a panic in the core's own code is told to the
    // host before the core stops. Both hooks are the process's on a device, so
    // what is not the core's own (another thread's, or the host's while the
    // core is out on `link_out`) goes on as it would have.
    bug::set_report(|errnum, at, n1, n2| {
        if in_guest() {
            let _ = stop::stopped(errnum as i32, at.file(), at.line(), n1, n2);
        }
    });
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !in_guest() {
            return prev(info);
        }
        // A bug's own panic, which its report told already.
        if info.payload().is::<bug::Bug>() {
            return;
        }
        let (file, line) = info.location().map_or(("", 0), |at| (at.file(), at.line()));
        let _ = stop::stopped(Errno::ENOTRECOVERABLE as i32, file, line, 0, 0);
    }));
    layout::install();
    export::mount();
    // The renderer's stream, as large as the manifest says, and the device it runs on.
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
    // A refused file that changed goes back to its owner, which reads it
    // again: the settings from their next keep on, the objects' file now.
    while let Some(r) = file::take_changed() {
        match r {
            file::Refused::Storage => settings_emptied(),
            file::Refused::Objects(id) => _ = repository::on_emptied(id),
        }
    }
    if let Some(at) = request::next_pump_at(now()) {
        invalidate_at(at);
    }
    api::gpu_flush();
    post_ui(state());
    executor::pending()
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
            OrientationCommand::Listen => start(Sensor::Orientation),
            OrientationCommand::Remove => stop(Sensor::Orientation),
        }
    }
    if let Some((azimuth, m_per_px)) = s.last_snapshot.take_view_changed() {
        abi::overlay::view(azimuth, m_per_px).or_bug(Errno::EPROTO);
    }
    if let Some(shown) = s.last_snapshot.take_changed() {
        abi::overlay::show(&ui::snapshot(&shown)).or_bug(Errno::EPROTO);
    }
}
