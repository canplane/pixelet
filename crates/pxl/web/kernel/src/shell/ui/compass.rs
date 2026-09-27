// pxl-kernel/shell/ui/compass.rs

//! How the compass shows, and what its reset does.

use core::cell::Cell;

use crate::abi::cmd::CompassState;
use crate::control::camera::input::dragging;
use crate::control::camera::store::{camera_store, CameraStatus};
use crate::layer::terrain::ground_snap::reground_map_center;

thread_local! {
    /// Whether the compass was shown at the last reading, which the snapshot
    /// takes once a turn.
    static SHOWN: Cell<bool> = const { Cell::new(false) };
}

/// Following while the map follows the device's heading, where north is what
/// one most wants to see and turning north up is not the compass's to do.
/// Otherwise shown once the map is turned past a threshold that narrows as
/// the camera pulls back, and kept while the map is in hand — dragged,
/// pinched, wheeled, or still moving from any of them or from a reset — so it
/// never goes from under a finger turning the map back through north. A map
/// in hand but never turned does not show it. How long a change must stand
/// before the button follows is the UI's (feature/map/ui/control/compass.tsx).
pub fn compass_state() -> CompassState {
    let camera = camera_store();
    if camera.is_follow_mode {
        SHOWN.set(false);
        return CompassState::Following;
    }
    let (dist, azimuth) = (camera.view_state.dist, camera.view_state.azimuth);
    let azimuth_thres = (25.0 / (dist + 2.5)).max(3.0).to_radians();
    let in_hand = dragging() || camera.wheel_zooming || camera.camera_status != CameraStatus::Idle;
    let shown = azimuth.abs() > azimuth_thres || (SHOWN.get() && in_hand);
    SHOWN.set(shown);
    if shown {
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
    if camera_store().is_follow_mode {
        return;
    }
    camera_store().reset_azimuth();
    reground_map_center();
}
