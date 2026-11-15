// pxl-core/link/export.rs

//! The interfaces the core serves (spec/client/core/wit/), each one's methods
//! in its own module beside this one, served by one object, `Core`, which
//! `mount` mounts under each interface's named handle. Its GPU surfaces
//! (surface.rs) are xpute's, and come apart.

mod camera;
mod edit;
pub(super) mod input;
mod perf;
mod scene;
mod sensor;
mod setting;
mod source;
mod surface;

use std::rc::Rc;

use crate::abi;
use crate::domain::control::input::forget;
use crate::sys::sched::invalidate;
use xpute_guest::abi::life;
use xpute_kit::status::errno::Errno;

/// Refuses what the decoder cannot check: a value outside the range the core
/// takes, or a handle the core does not hold. Nothing past the link checks
/// these again.
fn or_refuse<T>(v: Option<T>, errnum: Errno) -> Result<T, Errno> {
    v.ok_or(errnum)
}

fn refuse_unless(valid: bool, errnum: Errno) -> Result<(), Errno> {
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

/// Answers a method applied: the next frame shows it, and the entry's end
/// posts to the UI (link.rs `end`).
fn applied() -> Result<(), Errno> {
    invalidate();
    Ok(())
}

/// What serves the core's interfaces.
struct Core;

/// Mounts the interfaces the core serves.
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
    fn track(&self, tracking: abi::camera::Tracking) -> Result<(), Errno> {
        camera::track(tracking)
    }
    fn north_up(&self) -> Result<(), Errno> {
        camera::north_up()
    }
    fn goto(&self, lon: f64, lat: f64) -> Result<(), Errno> {
        camera::goto(lon, lat)
    }
}

impl abi::input::Guest for Core {
    fn pointer(
        &self,
        phase: abi::input::PointerPhase,
        touch: bool,
        id: i32,
        drag: abi::input::Drag,
        x: f64,
        y: f64,
    ) -> Result<(), Errno> {
        input::pointer(phase, touch, id, drag, x, y)
    }
    fn wheel(&self, delta_y: f64) -> Result<(), Errno> {
        input::wheel(delta_y)
    }
    fn pinch(&self, scale: f64) -> Result<(), Errno> {
        input::pinch(scale)
    }
    fn r#move(&self, x: i32, y: i32) -> Result<(), Errno> {
        input::r#move(x, y)
    }
    fn xr_pose(
        &self,
        x: f64,
        y: f64,
        z: f64,
        qx: f64,
        qy: f64,
        qz: f64,
        qw: f64,
    ) -> Result<(), Errno> {
        input::xr_pose(x, y, z, qx, qy, qz, qw)
    }
    fn xr_pose_end(&self) -> Result<(), Errno> {
        input::xr_pose_end()
    }
}

impl abi::scene::Guest for Core {
    fn draw(
        &self,
        surface: u32,
        content: abi::scene::SceneContent,
        group: Option<u8>,
        bg: Option<u32>,
    ) -> Result<Result<(), abi::types::ErrorCode>, Errno> {
        scene::draw(surface, content, group, bg)
    }
    fn chat(&self, text: String) -> Result<Result<(), abi::types::ErrorCode>, Errno> {
        scene::chat(text)
    }
}

impl abi::sensor::Guest for Core {
    fn pos(&self, at: Option<abi::sensor::Position>) -> Result<(), Errno> {
        sensor::pos(at)
    }
    fn heading(&self, deg: Option<f64>) -> Result<(), Errno> {
        sensor::heading(deg)
    }
    fn location(&self, status: abi::sensor::LocationStatus) -> Result<(), Errno> {
        sensor::location(status)
    }
}

impl abi::setting::Guest for Core {
    fn dev(&self, dev: bool) -> Result<(), Errno> {
        setting::dev(dev)
    }
    fn set(&self, key: u32, val: u32) -> Result<Result<(), abi::types::ErrorCode>, Errno> {
        setting::set(key, val)
    }
}

impl abi::edit::Guest for Core {
    fn hold(&self, tool: abi::edit::Tool, val: u32, scale_step: i32) -> Result<(), Errno> {
        edit::hold(tool, val, scale_step)
    }
    fn marker_text(&self, text: String) -> Result<Result<(), abi::types::ErrorCode>, Errno> {
        edit::marker_text(text)
    }
    fn marker_cancel(&self) -> Result<(), Errno> {
        edit::marker_cancel()
    }
    fn marker_commit(&self) -> Result<(), Errno> {
        edit::marker_commit()
    }
}

impl abi::source::Guest for Core {
    fn declare(&self, min_zoom: u32, formats: Vec<u32>, shards: Vec<String>) -> Result<(), Errno> {
        source::declare(min_zoom, formats, shards)
    }
}

impl abi::perf::Guest for Core {
    fn start(&self) -> Result<Result<(), abi::types::ErrorCode>, Errno> {
        perf::start()
    }
    fn stop(&self) -> Result<(), Errno> {
        perf::stop()
    }
    fn stats(&self, window_ms: Option<f64>) -> Result<Vec<u8>, Errno> {
        perf::stats(window_ms)
    }
    fn table(&self, table: u32, from: u32) -> Result<Vec<u8>, Errno> {
        perf::table(table, from)
    }
    fn strtab(&self) -> Result<Vec<u8>, Errno> {
        perf::strtab()
    }
}

impl life::Guest for Core {
    fn hidden(&self) -> Result<(), Errno> {
        super::life::on_hidden();
        Ok(())
    }
    fn shown(&self) -> Result<(), Errno> {
        super::life::on_shown();
        Ok(())
    }
    /// A UI attached, the first or one a reload makes on the same core: it is
    /// posted the overlay whole at the entry's end (link.rs `tell_all`), and
    /// starts with no pointer pressed.
    fn attached(&self) -> Result<(), Errno> {
        super::tell_all();
        forget();
        Ok(())
    }
}

#[cfg(test)]
#[path = "export.test.rs"]
mod test;
