// pxl-core/link/export.rs

//! The interfaces the core implements (spec/client/core/wit/), each one's
//! methods in its own module beside this one, implemented by one object,
//! `Core`, which `mount` mounts under each interface's named handle. Its GPU
//! surfaces (surface.rs) are xpute's, and come apart.

mod camera;
mod edit;
pub(super) mod input;
mod perf;
mod scene;
mod sensor;
mod setting;
mod source;
mod surface;

use crate::spec::step::InvalidationReason;
use std::rc::Rc;

use crate::abi;
use crate::abi::types::TrackingMode;
use crate::domain::control::input::forget;
use crate::domain::control::navigation::tracking_mode;
use crate::sys::sched::invalidate;
use xpute_guest::abi::life;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;

/// Refuses what the decoder cannot check: a value outside the range the core
/// takes, or a handle the core does not hold. Nothing past the link checks
/// these again.
#[track_caller]
fn or_refuse<T>(v: Option<T>, errnum: Errno) -> Result<T, XputeError> {
    match v {
        Some(v) => Ok(v),
        None => Err(XputeError::new(errnum)),
    }
}

#[track_caller]
fn refuse_unless(valid: bool, errnum: Errno) -> Result<(), XputeError> {
    or_refuse(valid.then_some(()), errnum)
}

fn on_earth(lon: f64, lat: f64) -> bool {
    (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat)
}

/// Whether `deg` is absent or a bearing in degrees, as a compass or a course
/// gives it.
fn bearing(deg: Option<f64>) -> bool {
    deg.is_none_or(|d| (0.0..=360.0).contains(&d))
}

/// A method's answer, which says whether the core took what the method
/// asked. A refusal says it did not.
trait Answer {
    fn accepts(&self) -> bool;
}

impl Answer for () {
    fn accepts(&self) -> bool {
        true
    }
}

impl<T: Answer, E> Answer for Result<T, E> {
    fn accepts(&self) -> bool {
        self.as_ref().is_ok_and(Answer::accepts)
    }
}

/// Asks for the frame that shows what a method from outside changed. What
/// comes from outside asks here, where the method is answered, as a method
/// (`InvalidationReason::Method`); what changes inside the core asks where it
/// changes, by its own reason (sys/sched.rs `invalidate`). Where a method's
/// state asks as well, both are asks for the one frame.
fn ask() {
    invalidate(InvalidationReason::Method);
}

/// Hands on a method's answer, asking for its frame where the answer accepts
/// the method. Which methods ask is written where the interfaces are
/// implemented, below and for a surface in surface.rs: every method of the
/// UI's that changes what the core holds, and none that only reads. The
/// methods' bodies, each in its interface's module, ask for nothing.
fn with_frame<A: Answer>(answer: A) -> A {
    if answer.accepts() {
        ask();
    }
    answer
}

/// What implements the core's interfaces.
struct Core;

/// Mounts the interfaces the core implements.
pub(super) fn mount() {
    abi::camera::mount(Rc::new(Core));
    abi::input::mount(Rc::new(Core));
    abi::scene::mount(Rc::new(Core));
    abi::sensor::mount(Rc::new(Core));
    abi::setting::mount(Rc::new(Core));
    abi::edit::mount(Rc::new(Core));
    abi::source::mount(Rc::new(Core));
    abi::perf::mount(Rc::new(Core));
    surface::mount();
    life::mount(Rc::new(Core));
}

impl abi::camera::Guest for Core {
    fn track(&self, mode: TrackingMode) -> Result<(), XputeError> {
        with_frame(camera::track(mode))
    }
    fn goto(&self, lon: f64, lat: f64) -> Result<(), XputeError> {
        with_frame(camera::goto(lon, lat))
    }
}

impl abi::input::Guest for Core {
    fn pan(&self, dx: f64, dy: f64) -> Result<(), XputeError> {
        with_frame(input::pan(dx, dy))
    }
    fn rotate(&self, rad: f64) -> Result<(), XputeError> {
        with_frame(input::rotate(rad))
    }
    fn tilt(&self, rad: f64) -> Result<(), XputeError> {
        with_frame(input::tilt(rad))
    }
    fn zoom(&self, scale: f64) -> Result<(), XputeError> {
        with_frame(input::zoom(scale))
    }
    fn drag(&self, dragging: bool) -> Result<(), XputeError> {
        with_frame(input::drag(dragging))
    }
    fn tap(&self, x: f64, y: f64) -> Result<(), XputeError> {
        with_frame(input::tap(x, y))
    }
    fn hover(&self, x: f64, y: f64) -> Result<(), XputeError> {
        with_frame(input::hover(x, y))
    }
    fn out(&self) -> Result<(), XputeError> {
        with_frame(input::out())
    }
    fn r#move(&self, x: i32, y: i32) -> Result<(), XputeError> {
        with_frame(input::r#move(x, y))
    }
}

impl abi::scene::Guest for Core {
    fn draw(
        &self,
        surface: u32,
        content: abi::scene::SceneContent,
        group: Option<u8>,
        bg: Option<u32>,
    ) -> Result<Result<(), abi::types::ErrorCode>, XputeError> {
        with_frame(scene::draw(surface, content, group, bg))
    }
    fn chat(&self, text: String) -> Result<Result<(), abi::types::ErrorCode>, XputeError> {
        with_frame(scene::chat(text))
    }
}

impl abi::sensor::Guest for Core {
    fn pos(&self, at: Option<abi::sensor::Position>) -> Result<(), XputeError> {
        with_frame(sensor::pos(at))
    }
    /// The one method that asks by a condition: the device tells its heading
    /// far more often than a frame is worth, and the heading turns the camera
    /// only while it is tracked with the user's location, so it asks only
    /// then.
    fn heading(&self, deg: Option<f64>) -> Result<(), XputeError> {
        let answer = sensor::heading(deg);
        if tracking_mode() == TrackingMode::FollowWithHeading {
            return with_frame(answer);
        }
        answer
    }
    fn location(&self, status: abi::sensor::LocationStatus) -> Result<(), XputeError> {
        with_frame(sensor::location(status))
    }
}

impl abi::setting::Guest for Core {
    fn set(&self, key: u32, val: u32) -> Result<Result<(), abi::types::ErrorCode>, XputeError> {
        with_frame(setting::set(key, val))
    }
}

impl abi::edit::Guest for Core {
    fn hold(&self, tool: abi::edit::Tool, val: u32, scale_step: i32) -> Result<(), XputeError> {
        with_frame(edit::hold(tool, val, scale_step))
    }
    fn marker_text(&self, text: String) -> Result<Result<(), abi::types::ErrorCode>, XputeError> {
        with_frame(edit::marker_text(text))
    }
    fn marker_cancel(&self) -> Result<(), XputeError> {
        with_frame(edit::marker_cancel())
    }
    fn marker_commit(&self) -> Result<(), XputeError> {
        with_frame(edit::marker_commit())
    }
}

impl abi::source::Guest for Core {
    fn declare(
        &self,
        min_zoom: u32,
        formats: Vec<u32>,
        shards: Vec<String>,
    ) -> Result<(), XputeError> {
        with_frame(source::declare(min_zoom, formats, shards))
    }
}

impl abi::perf::Guest for Core {
    fn start(&self) -> Result<Result<(), abi::types::ErrorCode>, XputeError> {
        with_frame(perf::start())
    }
    fn stop(&self) -> Result<(), XputeError> {
        with_frame(perf::stop())
    }
    // The four below only read, and a read asks for no frame: it would
    // draw into what the profiler measures.
    fn stats(&self, window_ms: Option<f64>) -> Result<Vec<u8>, XputeError> {
        perf::stats(window_ms)
    }
    fn table(&self, table: u32, from: u64, cnt: u32) -> Result<abi::perf::Rows, XputeError> {
        perf::table(table, from, cnt)
    }
    fn worst(&self) -> Result<Vec<u8>, XputeError> {
        perf::worst()
    }
    fn strtab(&self) -> Result<Vec<u8>, XputeError> {
        perf::strtab()
    }
}

// The runtime's notices of the app's life are not the UI's methods and ask
// for no frame here: a save's completion asks after `hidden`, `shown` wakes
// the wait for the GPU's device, whose answer asks (link/gpu.rs), and
// `attached` is answered by the entry's post to the UI, the surface it then
// opens asking for its own.
impl life::Guest for Core {
    fn hidden(&self) -> Result<(), XputeError> {
        super::life::on_hidden();
        Ok(())
    }
    fn shown(&self) -> Result<(), XputeError> {
        super::life::on_shown();
        Ok(())
    }
    /// A UI attached, the first or one a reload makes on the same core: it is
    /// posted the overlay whole at the entry's end (link.rs `tell_all`), and
    /// starts with no pointer pressed.
    fn attached(&self) -> Result<(), XputeError> {
        super::tell_all();
        forget();
        Ok(())
    }
}

#[cfg(test)]
#[path = "export.test.rs"]
mod test;
