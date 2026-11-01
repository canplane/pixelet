// pxl-kernel/door/apply/camera.rs

//! CAMERA: the camera's: the tracking button, the compass, a search result.

use super::State;
use super::{ask_turn, given, on_earth, refuse_unless};
use crate::domain::control::camera::controller;
use crate::domain::control::camera::state::IntentPatch;
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::control::navigation::destination;
use crate::domain::control::navigation::follow_mode;
use crate::sys::sched::edge::request_world_tick;
use crate::turn::world_platform;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::CAMERA_TRACKING => {
            follow_mode::tracking_action(
                abi::CameraTracking::read(given(pkt)?)?.tracking,
                world_platform().orient,
            );
            request_world_tick();
            Ok(Some(0))
        }
        Command::CAMERA_NORTH_UP => {
            // Following the heading would turn it straight back.
            if !follow_mode::following() {
                controller::reset_azimuth();
            }
            ask_turn(s)
        }
        Command::CAMERA_GOTO => {
            let p = abi::CameraGoto::read(given(pkt)?)?;
            refuse_unless(on_earth(p.lon, p.lat), Errno::EINVAL)?;
            destination::go_to_deg(
                p.lon,
                p.lat,
                IntentPatch {
                    dist: p.dist,
                    azimuth: p.azimuth,
                    polar: p.polar,
                },
            );
            ask_turn(s)
        }
        _ => Ok(None),
    }
}
