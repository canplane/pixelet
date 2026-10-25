// pxl-kernel/domain/control/navigation/follow_mode.rs

use pxl_spec::space::coord::q3::Q3;
use xpute_kit::math::scalar::wrap;

use crate::domain::control::camera::controller::Motion;
use crate::domain::control::camera::state::IntentPatch;
use crate::domain::control::navigation::destination::go_to_ground;
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::control::sensor::reading::{device_location_store, device_orientation_store, DevicePoseStatus};
use crate::sys::sched::edge::wake_at;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

/// Per heading sample, not per frame: compass jitter (worse indoors) passes
/// the store's 1.5° dead zone, and this smooths it before it reaches azimuth.
const HEADING_SMOOTH_ALPHA: f64 = 0.25;

/// Longer than the geolocation retry delay, so a recoverable gap gets its
/// retry first.
const SENSOR_GRACE_MS: f64 = 12000.0;

struct Follow {
    /// Whether the camera follows the device: the controls turn it on and off,
    /// and a dead location gives it up.
    on: bool,
    smoothed_azimuth: Option<f64>,
    was_following: bool,
    last_device: Option<Q3>,
    last_heading: Option<f64>,
    /// The give-up deadline, and the location status it was set for.
    grace_due: Option<f64>,
    grace_for: Option<DevicePoseStatus>,
}

static FOLLOW: Global<Follow> = Global::new();

fn follow() -> &'static mut Follow {
    FOLLOW.get(|| Follow {
        on: false,
        smoothed_azimuth: None,
        was_following: false,
        last_device: None,
        last_heading: None,
        grace_due: None,
        grace_for: None,
    })
}

pub fn following() -> bool {
    follow().on
}

pub fn set_following(on: bool) {
    follow().on = on;
}

pub fn follow_mode_step() {
    let f = follow();
    let is_follow_mode = f.on;
    let heading = device_orientation_store().orientation.and_then(|o| o.heading);
    let device_coord = off_center().device_coord;

    // Follow needs the location READY; a heading only turns the camera where there
    // is one, so a device without a compass still follows where it is. A blip
    // must not drop it (it read as the mode switching itself off), but a dead
    // location must eventually, or the camera freezes behind a disabled button.
    let status = device_location_store().pose.status();
    let ready = status == DevicePoseStatus::Ready;
    if is_follow_mode && !ready {
        // Restarted whenever a status moves: LOADING to ERROR is a fresh gap with its
        // own retry ahead.
        if f.grace_for != Some(status) {
            f.grace_for = Some(status);
            f.grace_due = Some(now() + SENSOR_GRACE_MS);
        }
        match f.grace_due {
            Some(due) if now() >= due => {
                f.grace_due = None;
                f.grace_for = None;
                f.on = false;
            }
            Some(due) => wake_at(due),
            None => {}
        }
    } else {
        f.grace_due = None;
        f.grace_for = None;
    }

    // Dropped on exit, so re-entering snaps to the heading rather than easing
    // from a stale one.
    let entered = is_follow_mode && !f.was_following;
    if !is_follow_mode {
        f.smoothed_azimuth = None;
    }
    f.was_following = is_follow_mode;
    if !is_follow_mode {
        return;
    }

    // No EMA here: the smooth move is the one smoothing stage. On the floor nearest
    // the fix's altitude.
    if let Some(device) = device_coord.filter(|d| entered || Some(*d) != f.last_device) {
        go_to_ground(device, Motion::Smooth);
    }
    f.last_device = device_coord;

    if let Some(heading) = heading.filter(|h| entered || Some(*h) != f.last_heading) {
        let raw_azimuth = heading * (core::f64::consts::PI / 180.0);
        let smoothed = match f.smoothed_azimuth {
            None => raw_azimuth,
            // The short way across the wrap.
            Some(s) => s + wrap(raw_azimuth - s, -core::f64::consts::PI, core::f64::consts::PI) * HEADING_SMOOTH_ALPHA,
        };
        f.smoothed_azimuth = Some(smoothed);
        crate::domain::control::camera::controller::set_intent(IntentPatch {
            azimuth: Some(smoothed),
            ..Default::default()
        });
    }
    f.last_heading = heading;
}
