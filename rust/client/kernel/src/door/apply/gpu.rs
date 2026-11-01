// pxl-kernel/door/apply/gpu.rs

//! GPU: the GPU's: a device answered.

use super::State;
use crate::domain::control::cmd::Command;
use crate::sys::sched::edge::request_world_tick;
use pxl_render::gpu::api;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    _s: &mut State,
    cmd: Command,
    _pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::GPU_READY => {
            api::on_device_answered();
            request_world_tick();
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}
