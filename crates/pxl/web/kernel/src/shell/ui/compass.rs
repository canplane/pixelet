// pxl-kernel/shell/ui/compass.rs

//! How the compass shows, and what its reset does.

use crate::abi::cmd::CompassState;
use crate::control::camera::input::dragging;
use crate::control::camera::store::{camera_store, CameraStatus};
use crate::global::Global;

/// Whether the compass was shown at the last reading, which the snapshot
/// takes once a turn.
static SHOWN: Global<bool> = Global::new();

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
    let azimuth = camera.intent.azimuth;
    let azimuth_thres = (25.0 / (crate::control::camera::controller::radius() + 2.5)).max(3.0).to_radians();
    let in_hand = dragging() || crate::control::camera::controller::zooming() || camera.camera_status != CameraStatus::Idle;
    let shown = azimuth.abs() > azimuth_thres || (*SHOWN.get(|| false) && in_hand);
    SHOWN.set(shown);
    if shown {
        CompassState::Shown
    } else {
        CompassState::Hidden
    }
}

/// North up. Nothing while the map follows the heading, which turns it
/// straight back: north up is the tracking button's to ask then, and the
/// compass is shown dimmed.
pub fn compass_reset() {
    if camera_store().is_follow_mode {
        return;
    }
    crate::control::camera::controller::reset_azimuth();
}
