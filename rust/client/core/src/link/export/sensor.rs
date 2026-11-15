// pxl-core/link/export/sensor.rs

//! The sensor interface's methods: the device's position, its heading and the
//! location's status, each as the UI tells it on a change.

use super::{applied, bearing, on_earth, refuse_unless};
use crate::abi::sensor::{LocationStatus, Position};
use crate::domain::control::navigation::follow_mode::following;
use crate::sys::sensor;
use xpute_kit::status::errno::Errno;

/// The location indicator stands at the position, so a position that moves
/// or goes asks for a frame.
pub(super) fn pos(at: Option<Position>) -> Result<(), Errno> {
    refuse_unless(
        at.as_ref().is_none_or(|p| on_earth(p.lon, p.lat)),
        Errno::EINVAL,
    )?;
    sensor::set_pos(at);
    applied()
}

/// The heading turns the camera only while it follows the device, so a
/// changed heading asks for a frame only then.
pub(super) fn heading(deg: Option<f64>) -> Result<(), Errno> {
    refuse_unless(bearing(deg), Errno::EINVAL)?;
    sensor::set_heading(deg);
    if following() {
        return applied();
    }
    Ok(())
}

/// While following, a location that is not ready starts follow mode's grace
/// (`follow_mode_step`), whose deadline then wakes its own frame; a ready one
/// ends it at the next frame.
pub(super) fn location(status: LocationStatus) -> Result<(), Errno> {
    sensor::set_location_status(status);
    if following() && status != LocationStatus::Ready {
        return applied();
    }
    Ok(())
}
