// pxl-core/link/export/camera.rs

//! The camera interface's methods: the location button, and going to a search
//! result.

use super::{on_earth, refuse_unless};
use crate::abi::types::LocationMode;
use crate::domain::control::navigation;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;

pub(super) fn set_location_mode(mode: LocationMode) -> Result<(), XputeError> {
    navigation::set_location_mode(mode);
    Ok(())
}

pub(super) fn goto(lon: f64, lat: f64) -> Result<(), XputeError> {
    refuse_unless(on_earth(lon, lat), Errno::EINVAL)?;
    navigation::go_to_deg(lon, lat);
    Ok(())
}
