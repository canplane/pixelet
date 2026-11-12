// pxl-core/door/apply/camera.rs

//! The CAMERA major's commands: the tracking button, the compass, and going
//! to a search result.

use super::{applied, on_earth, refuse_unless};
use crate::domain::control::camera::controller;
use crate::domain::control::camera::state::IntentPatch;
use crate::domain::control::navigation::destination;
use crate::domain::control::navigation::follow_mode;
use crate::abi::camera::Tracking;
use crate::sys::sched::invalidate;
use xpute_kit::status::errno::Errno;

pub(super) fn track(tracking: Tracking) -> Result<(), Errno> {
    follow_mode::tracking_action(tracking);
    invalidate();
    Ok(())
}

pub(super) fn north_up() -> Result<(), Errno> {
    // While the camera follows the device's heading, a reset azimuth would
    // turn straight back, so it is left alone.
    if !follow_mode::following() {
        controller::reset_azimuth();
    }
    applied()
}

pub(super) fn goto(lon: f64, lat: f64, dist: Option<f64>, azimuth: Option<f64>, polar: Option<f64>) -> Result<(), Errno> {
    refuse_unless(on_earth(lon, lat), Errno::EINVAL)?;
    destination::go_to_deg(
        lon,
        lat,
        IntentPatch {
            dist,
            azimuth,
            polar,
        },
    );
    applied()
}
