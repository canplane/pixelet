// pxl_core/module/map/ui/control/compass.rs

//! Whether the compass shows, and what its reset does.

use crate::module::camera::store::camera_store;
use crate::module::world::terrain::ground_snap::reground_map_center;

/// Shown once the map is turned past a threshold that narrows as the camera
/// pulls back, and never in follow mode.
pub fn show_compass() -> bool {
    let camera = camera_store();
    let (dist, azimuth) = (camera.view_state.dist, camera.view_state.azimuth);
    let azimuth_thres = (25.0 / (dist + 2.5)).max(3.0).to_radians();
    azimuth.abs() > azimuth_thres && !camera.is_follow_mode
}

/// reset_azimuth doesn't re-sample elevation, so map_center can be left stale
/// relative to the new angle if it isn't re-grounded alongside it.
pub fn compass_reset() {
    camera_store().reset_azimuth();
    reground_map_center();
}
