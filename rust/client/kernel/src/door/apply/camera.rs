// pxl-kernel/door/apply/camera.rs

//! CAMERA: the camera's: the tracking button, the compass, a search result.

use super::State;
use super::{ask_turn, on_earth, refuse_unless};
use crate::domain::control::camera::controller;
use crate::domain::control::camera::state::IntentPatch;
use crate::domain::control::navigation::destination;
use crate::domain::control::navigation::follow_mode;
use crate::door::cmd as abi;
use crate::sys::sched::edge::request_world_tick;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn tracking(c: abi::CameraTracking) -> Outcome {
    follow_mode::tracking_action(c.tracking);
    request_world_tick();
    Ok(Some(0))
}

pub(super) fn north_up(s: &mut State) -> Outcome {
    // Following the heading would turn it straight back.
    if !follow_mode::following() {
        controller::reset_azimuth();
    }
    ask_turn(s)
}

pub(super) fn goto(s: &mut State, c: abi::CameraGoto) -> Outcome {
    refuse_unless(on_earth(c.lon, c.lat), Errno::EINVAL)?;
    destination::go_to_deg(
        c.lon,
        c.lat,
        IntentPatch {
            dist: c.dist,
            azimuth: c.azimuth,
            polar: c.polar,
        },
    );
    ask_turn(s)
}
