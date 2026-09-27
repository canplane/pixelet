// pxl-kernel/shell/ui/compass.rs

//! How the compass shows, and what its reset does.

use crate::abi::cmd::CompassState;
use crate::control::camera::store::camera_store;
use crate::layer::terrain::ground_snap::reground_map_center;

/// Following while the map follows the device's heading, where north is what
/// one most wants to see and turning north up is not the compass's to do;
/// otherwise shown once the map is turned past a threshold that narrows as
/// the camera pulls back.
pub fn compass_state() -> CompassState {
    let camera = camera_store();
    if camera.is_follow_mode {
        return CompassState::Following;
    }
    let (dist, azimuth) = (camera.view_state.dist, camera.view_state.azimuth);
    let azimuth_thres = (25.0 / (dist + 2.5)).max(3.0).to_radians();
    if azimuth.abs() > azimuth_thres {
        CompassState::Shown
    } else {
        CompassState::Hidden
    }
}

/// reset_azimuth doesn't re-sample elevation, so map_center can be left stale
/// relative to the new angle if it isn't re-grounded alongside it. Nothing
/// while the map follows the heading, which turns it straight back: north up
/// is the tracking button's to ask then, and the compass is shown dimmed.
pub fn compass_reset() {
    if compass_state() == CompassState::Following {
        return;
    }
    camera_store().reset_azimuth();
    reground_map_center();
}
