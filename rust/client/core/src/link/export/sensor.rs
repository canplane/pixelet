// pxl-core/link/export/sensor.rs

//! The sensor interface's methods: the device's position, its heading and the
//! location's status, each as the UI tells it on a change.

use super::{bearing, on_earth, refuse_unless};
use crate::abi::sensor::{LocationStatus, Position};
use crate::sys::sensor;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;

pub(super) fn pos(at: Option<Position>) -> Result<(), XputeError> {
    refuse_unless(
        at.as_ref().is_none_or(|p| on_earth(p.lon, p.lat)),
        Errno::EINVAL,
    )?;
    sensor::set_pos(at);
    Ok(())
}

pub(super) fn heading(deg: Option<f64>) -> Result<(), XputeError> {
    refuse_unless(bearing(deg), Errno::EINVAL)?;
    sensor::set_heading(deg);
    Ok(())
}

pub(super) fn location(status: LocationStatus) -> Result<(), XputeError> {
    sensor::set_location_status(status);
    Ok(())
}
