// pxl-core/door/apply.rs

//! Applies the commands the UI's protocol carries, each major's commands in
//! its own module beside this one. It also applies a surface lent
//! (`on_surface`), which xpute-gpu decodes, and the xpute commands the door
//! hands over already decoded: a turn's pacing (turn.rs `turn`) and a changed file (fs.rs `changed`).

mod camera;
mod edit;
pub(super) mod fs;
pub(super) mod input;
mod perf;
mod scene;
mod sensor;
mod setting;
mod source;
mod surface;
pub(super) mod turn;

use super::State;
use crate::door::cmd::{self as abi, ui};
use crate::sys::sched::request_world_tick;
use pxl_render::gpu::api;
use xpute_core::io::door::Outcome;
use xpute_gpu::abi as gpu;
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
    request_world_tick();
    s.snapshot_due = true;
    Ok(Some(0.into()))
}

/// The GPU's commands, under the major the program gives it: the device's
/// answer (GPU_READY), and a surface the UI lends the GPU (GPU_SURFACE_OPEN,
/// _RESIZE, _CLOSE).
impl gpu::Exports for State {
    /// The GPU answers, the first time or again after a loss. The core then
    /// asks for the device's block and makes the renderer on it once the block
    /// is in (pxl-render gpu/api.rs). A later loss is reported through the
    /// block and read at the start of a turn.
    fn gpu_ready(&mut self) -> Outcome {
        api::on_device_answered();
        request_world_tick();
        Ok(Some(0.into()))
    }
    fn gpu_surface_open(&mut self) -> Outcome {
        surface::open()
    }
    fn gpu_surface_resize(&mut self, c: gpu::GpuSurfaceResize) -> Outcome {
        surface::resize(c)
    }
    fn gpu_surface_close(&mut self, c: gpu::GpuSurfaceClose) -> Outcome {
        surface::close(c)
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
    fn setting_dev(&mut self, c: abi::SettingDev) -> Outcome {
        setting::dev(c)
    }
    fn sensor_locate(&mut self) -> Outcome {
        sensor::locate(self)
    }
    fn source_declare(&mut self, c: abi::SourceDeclare<'_>) -> Outcome {
        source::declare(c)
    }
    fn input_typing(&mut self, c: abi::InputTyping) -> Outcome {
        input::typing(c)
    }
    fn input_pointer(&mut self, c: abi::InputPointer) -> Outcome {
        input::pointer(c)
    }
    fn input_wheel(&mut self, c: abi::InputWheel) -> Outcome {
        input::wheel(c)
    }
    fn input_pinch(&mut self, c: abi::InputPinch) -> Outcome {
        input::pinch(c)
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
    fn perf_rings(&mut self, c: abi::PerfRings) -> Outcome {
        perf::rings(c)
    }
    fn perf_strtab(&mut self) -> Outcome {
        perf::strtab()
    }
}

#[cfg(test)]
#[path = "apply.test.rs"]
mod test;
