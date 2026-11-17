// pxl-core/link/export/camera.rs

//! The camera interface's methods: the tracking button, the compass, and going
//! to a search result.

use super::{on_earth, refuse_unless};
use crate::abi::camera::Tracking;
use crate::domain::control::camera::controller;
use crate::domain::control::navigation;
use crate::domain::control::navigation::follow_mode;
use xpute_kit::status::errno::Errno;

pub(super) fn track(tracking: Tracking) -> Result<(), Errno> {
    follow_mode::tracking_action(tracking);
    Ok(())
}

pub(super) fn north_up() -> Result<(), Errno> {
    // While the camera follows the device's heading, a reset azimuth would
    // turn straight back, so it is left alone.
    if !follow_mode::following() {
        controller::reset_azimuth();
    }
    Ok(())
}

pub(super) fn goto(lon: f64, lat: f64) -> Result<(), Errno> {
    refuse_unless(on_earth(lon, lat), Errno::EINVAL)?;
    navigation::go_to_deg(lon, lat);
    Ok(())
}
