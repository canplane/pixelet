// pxl-core/link.rs

//! The core's side of the link: its boot, and the start and end of every
//! entry, which xpute's link runs (lib.rs `link!`). The boot mounts what the
//! core implements and starts its tasks; the start begins the entry's budget;
//! the end runs the core's tasks until none is ready, flushes what they
//! recorded for the GPU, a frame's stream among it, and tells the UI what
//! changed, and the overlay whole to a UI that has just started
//! (`tell_all`).

pub mod bound;
mod export;
mod gpu;
pub mod layout;
mod life;
mod param;

use crate::abi;
use crate::domain::layer::object::repository;
use crate::domain::scene::character::surface::character_surface_close;
use crate::domain::setting::{on_settings_set_aside, settings_open};
use crate::domain::view::demand;
use crate::frame;
use crate::perf::record::perf_record;
use crate::perf::task::TaskRecord;
use crate::spec::step::InvalidationReason;
use crate::sys::io::dispatch;
use crate::sys::io::file;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::sched::executor;
use crate::sys::sched::{self, invalidate, invalidate_at};
use crate::ui;
use crate::ui::{SnapshotInput, SnapshotMemo};
use core::future::poll_fn;
use core::task::{Poll, Waker};
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
    /// What the UI is posted next, until `ui_task` takes it.
    ui_changed: Option<Changed>,
    /// The waker of `ui_task` while it posts the UI nothing.
    ui_waker: Option<Waker>,
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        last_snapshot: Default::default(),
        ui_changed: None,
        ui_waker: None,
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
    api::install(stream::SZ);
    gpu::boot();
    sched::boot();
    executor::spawner().spawn(ui_task().or_bug(Errno::ENOSPC));
    // A layer asks for no more tiles at once than its tile cache has slots.
    request::open(|c| demand::of_stream(c).map_or(0, |l| l.slot_cnt));
    // The settings' slot is opened here and read by the frames that follow.
    // What waits on the settings, the profiler and the snapshot, starts once
    // they are in (frame.rs). The object layer is read a shard at a time as the
    // view asks for them (domain/layer/object/repository.rs).
    settings_open();
    // The UI must hold the core's state from its first render on.
    invalidate(InvalidationReason::Pending);
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
/// events on, arms the next pump, flushes the GPU's stream where the host
/// admits a flush, a frame's in the entry that drew it, which the frame's end
/// has spent (sys/sched.rs `end_frame`), and posts to the UI what changed,
/// and answers whether any of that readied a task.
pub fn end() -> bool {
    // A device loss, which the host wrote into the device's block, is read
    // before the tasks run.
    if api::take_device_lost() {
        renderer::on_gpu_lost();
        loader::on_images_lost();
    }
    // The device finished an image, a pipeline or the atmosphere's measures,
    // and a frame must take it in, or the room a write put off waits for is
    // back. Both are taken, each once. A read the profiler takes is taken
    // here, and asks for no frame.
    let complete = renderer::complete();
    let retry = api::take_retry();
    if complete {
        invalidate(InvalidationReason::Completion);
    }
    if retry {
        invalidate(InvalidationReason::Pending);
    }
    renderer::take_reads();
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
    // theirs from their next keep on, and the object file is read again now.
    while let Some(r) = file::take_set_aside() {
        match r {
            file::Refused::Storage => on_settings_set_aside(),
            file::Refused::Object(id) => _ = repository::on_set_aside(id),
        }
    }
    if let Some(at) = request::next_pump_at(now()) {
        invalidate_at(at, InvalidationReason::Pending);
    }
    api::gpu_flush();
    post_ui(state());
    executor::pending()
}

/// Tells a UI that has just started where everything stands, at this
/// entry's end: the snapshot and the view whole, as nothing was posted to
/// it. The runtime tells the core each UI that attaches (xpute's
/// `life.attached`), the first and one a reload makes on the same core alike.
fn tell_all() {
    state().last_snapshot = SnapshotMemo::default();
}

/// What the UI is posted next: the view, its azimuth and meters per pixel,
/// and the snapshot's inputs with the text of a draft that opened before the
/// snapshot that shows it open (`SnapshotMemo`).
type Changed = (
    Option<(f64, f64)>,
    Option<(SnapshotInput, Option<&'static str>)>,
);

/// Tells the other side what changed, at an entry's end, where the UI is
/// posted nothing now (`ui_task`): what is drawn over the world, the view and
/// the snapshot.
fn post_ui(s: &mut State) {
    let Some(w) = s.ui_waker.take() else {
        return;
    };
    let changed = (
        s.last_snapshot.take_view_changed(),
        s.last_snapshot.take_changed(),
    );
    if changed.0.is_none() && changed.1.is_none() {
        s.ui_waker = Some(w);
        return;
    }
    s.ui_changed = Some(changed);
    w.wake();
}

/// Posts the UI what `post_ui` hands it, each method an `async func` sent
/// once the one before is answered, so a UI that lags is posted what stands
/// as it answers. A UI dropped meanwhile answers ECANCELED, and the next is
/// told all as it attaches (`tell_all`), so an answer is the UI's own.
#[embassy_executor::task]
async fn ui_task() {
    loop {
        let (view, changed) = poll_fn(|cx| {
            let s = state();
            match s.ui_changed.take() {
                Some(p) => Poll::Ready(p),
                None => {
                    s.ui_waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await;
        if let Some((azimuth, m_per_px)) = view {
            let _ = abi::overlay::view(azimuth, m_per_px).await;
        }
        if let Some((shown, draft)) = changed {
            if let Some(text) = draft {
                let _ = abi::overlay::draft(text).await;
            }
            let _ = abi::overlay::show(&ui::snapshot(&shown)).await;
        }
    }
}
