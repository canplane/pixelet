// pxl-core/door/apply.rs

//! Applies the commands the UI's protocol carries, each major's commands in
//! its own module beside this one. It also applies the xpute commands the
//! door hands over already decoded: a surface lent (`on_surface`), a turn's
//! pacing (turn.rs `turn`) and a changed file (fs.rs `changed`).

mod camera;
mod edit;
pub(super) mod fs;
mod input;
mod perf;
mod scene;
mod sensor;
mod setting;
mod surface;
pub(super) mod turn;

use super::State;
use crate::door::cmd::{self as abi, ui};
use crate::sys::sched::edge;
use xpute_core::ipc::door::{Outcome, Surface};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

/// Refuses what the decoder cannot check: a value outside the range the core
/// takes, or a handle the core does not hold. Nothing past the door checks
/// these again.
fn or_refuse<T>(v: Option<T>, errnum: Errno) -> Result<T, MarshalError> {
    v.ok_or_else(|| MarshalError::new(errnum))
}

fn refuse_unless(valid: bool, errnum: Errno) -> Result<(), MarshalError> {
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

/// Answers a UI command by asking for a turn and by marking this turn to
/// post to the runtime as it ends. That way a sensor close sent while turns
/// are paused is still heard.
fn ask_turn(s: &mut State) -> Outcome {
    edge::request_world_tick();
    s.snapshot_due = true;
    Ok(Some(0))
}

/// A surface the UI lends the GPU (GPU_SURFACE_OPEN, _RESIZE, _CLOSE).
pub(super) fn on_surface(c: Surface) -> Outcome {
    match c {
        Surface::Open => surface::open(),
        Surface::Resize(c) => surface::resize(c),
        Surface::Close(c) => surface::close(c),
    }
}

impl ui::Exports for State {
    fn sensor_pos(&mut self, c: abi::SensorPos) -> Outcome {
        sensor::pos(c)
    }
    fn sensor_orient(&mut self, c: abi::SensorOrient) -> Outcome {
        sensor::orient(c)
    }
    fn sensor_err(&mut self, c: abi::SensorErr) -> Outcome {
        sensor::err(c)
    }
    fn sensor_permission(&mut self, c: abi::SensorPermission) -> Outcome {
        sensor::permission(c)
    }
    fn sensor_platform(&mut self, c: abi::SensorPlatform) -> Outcome {
        sensor::platform(c)
    }
    fn setting_development(&mut self, c: abi::SettingDevelopment) -> Outcome {
        setting::development(c)
    }
    fn sensor_locate(&mut self) -> Outcome {
        sensor::locate(self)
    }
    fn turn_grant(&mut self, c: abi::TurnGrant) -> Outcome {
        turn::grant(c)
    }
    fn source_declare(&mut self, c: abi::SourceDeclare<'_>) -> Outcome {
        fs::declare(c)
    }
    fn input_pointer(&mut self, c: abi::InputPointer) -> Outcome {
        input::pointer(c)
    }
    fn input_wheel(&mut self, c: abi::InputWheel) -> Outcome {
        input::wheel(c)
    }
    fn input_move(&mut self, c: abi::InputMove) -> Outcome {
        input::r#move(c)
    }
    fn input_xr_pose(&mut self, c: abi::InputXrPose) -> Outcome {
        input::xr_pose(self, c)
    }
    fn input_xr_pose_end(&mut self) -> Outcome {
        input::xr_pose_end(self)
    }
    fn edit_tool(&mut self, c: abi::EditTool) -> Outcome {
        edit::tool(c)
    }
    fn edit_marker_text(&mut self, c: abi::EditMarkerText<'_>) -> Outcome {
        edit::marker_text(c)
    }
    fn edit_marker_cancel(&mut self) -> Outcome {
        edit::marker_cancel()
    }
    fn edit_marker_commit(&mut self) -> Outcome {
        edit::marker_commit()
    }
    fn scene_chat(&mut self, c: abi::SceneChat<'_>) -> Outcome {
        scene::chat(self, c)
    }
    fn scene_draw(&mut self, c: abi::SceneDraw) -> Outcome {
        scene::draw(self, c)
    }
    fn setting_set(&mut self, c: abi::SettingSet) -> Outcome {
        setting::set(self, c)
    }
    fn camera_tracking(&mut self, c: abi::CameraTracking) -> Outcome {
        camera::tracking(c)
    }
    fn camera_north_up(&mut self) -> Outcome {
        camera::north_up(self)
    }
    fn camera_goto(&mut self, c: abi::CameraGoto) -> Outcome {
        camera::goto(self, c)
    }
    fn perf_start(&mut self) -> Outcome {
        perf::start(self)
    }
    fn perf_stop(&mut self) -> Outcome {
        perf::stop(self)
    }
    fn perf_stats(&mut self, c: abi::PerfStats) -> Outcome {
        perf::stats(c)
    }
    fn perf_rings(&mut self) -> Outcome {
        perf::rings()
    }
}

#[cfg(test)]
#[path = "apply.test.rs"]
mod test;
