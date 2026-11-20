// pxl-core/link/export/camera.rs

//! The camera interface's methods: the tracking button, and going to a search
//! result.

use super::{on_earth, refuse_unless};
use crate::abi::types::TrackingMode;
use crate::domain::control::navigation;
use xpute_kit::status::errno::Errno;

pub(super) fn track(mode: TrackingMode) -> Result<(), Errno> {
    navigation::track(mode);
    Ok(())
}

pub(super) fn goto(lon: f64, lat: f64) -> Result<(), Errno> {
    refuse_unless(on_earth(lon, lat), Errno::EINVAL)?;
    navigation::go_to_deg(lon, lat);
    Ok(())
}
