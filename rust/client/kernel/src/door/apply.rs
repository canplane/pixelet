// pxl-kernel/door/apply.rs

//! The shell's commands, applied: each major's by its own module beside.

mod camera;
mod edit;
mod fs;
mod gpu;
mod input;
mod perf;
mod scene;
mod sensor;
mod setting;
mod shell;
mod surface;
mod sys;

use super::State;
use crate::domain::control::cmd::{self as abi, Command};
use crate::sys::sched::edge;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

fn given<'a>(pkt: &Option<&'a [u8]>) -> Result<&'a [u8], MarshalError> {
    or_refuse(*pkt, Errno::EBADMSG)
}

/// What the decoder cannot see, a value out of the range the kernel takes or a
/// handle it does not hold, is refused here: past the door nothing checks it again.
fn or_refuse<T>(v: Option<T>, errnum: Errno) -> Result<T, MarshalError> {
    v.ok_or_else(|| MarshalError::new(errnum))
}

fn refuse_unless(valid: bool, errnum: Errno) -> Result<(), MarshalError> {
    or_refuse(valid.then_some(()), errnum)
}

fn on_earth(lon: f64, lat: f64) -> bool {
    (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat)
}

/// A bearing in degrees, as a compass or a course gives it.
fn bearing(deg: Option<f64>) -> bool {
    deg.is_none_or(|d| (0.0..=360.0).contains(&d))
}

/// A UI command: asks for a turn, and has this one raise what it asks of the
/// host at its falling edge, so a sensor close sent after the strobe stopped is
/// still heard.
fn ask_turn(s: &mut State) -> Result<Option<i32>, MarshalError> {
    edge::request_world_tick();
    s.asked = true;
    Ok(Some(0))
}

/// Applies one submission, by its major's module. None (replied as ENOSYS)
/// for an unknown command or an event submitted as one.
pub(super) fn apply(
    s: &mut State,
    number: u32,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    let Some(cmd) = Command::of(number) else {
        return Ok(None);
    };
    match abi::Major::of(number >> 8) {
        Some(abi::Major::SYS) => sys::apply(s, cmd, pkt),
        Some(abi::Major::INPUT) => input::apply(s, cmd, pkt),
        Some(abi::Major::SENSOR) => sensor::apply(s, cmd, pkt),
        Some(abi::Major::FS) => fs::apply(s, cmd, pkt),
        Some(abi::Major::GPU) => gpu::apply(s, cmd, pkt),
        Some(abi::Major::SURFACE) => surface::apply(s, cmd, pkt),
        Some(abi::Major::SETTING) => setting::apply(s, cmd, pkt),
        Some(abi::Major::SCENE) => scene::apply(s, cmd, pkt),
        Some(abi::Major::CAMERA) => camera::apply(s, cmd, pkt),
        Some(abi::Major::PERF) => perf::apply(s, cmd, pkt),
        Some(abi::Major::EDIT) => edit::apply(s, cmd, pkt),
        Some(abi::Major::SHELL) => shell::apply(s, cmd, pkt),
        None => Ok(None),
    }
}

#[cfg(test)]
#[path = "apply.test.rs"]
mod test;
