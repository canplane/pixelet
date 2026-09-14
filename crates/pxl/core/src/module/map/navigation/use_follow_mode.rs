// pxl_core/module/map/navigation/use_follow_mode.rs

use pxl_shared::spec::space::pvf3::pvf3::PVF3;
use xpute_core::kit::math::wrap;

use super::use_map_navigation::use_map_navigation;
use crate::app::setting::store::use_settings_store;
use crate::global::{now, Global};
use crate::module::camera::store::{use_camera_store, ViewPatch};
use crate::module::device::pose::store::{use_device_location_store, use_device_orientation_store, DevicePoseStatus};
use crate::module::map::store::use_map_store;
use crate::module::world::terrain::elevation::ground_or_raw;

/// EMA factor applied per heading sample (not per frame — device orientation
/// events don't arrive frame-locked). Compass heading is inherently noisy
/// (magnetometer jitter, worse indoors/near metal), and the dead-zone filter in
/// the pose store only screens out sub-1.5° noise — this smooths what gets
/// through before it ever reaches azimuth, on top of (not instead of) the view
/// frame's per-frame ROT_DAMP easing.
const HEADING_SMOOTH_ALPHA: f64 = 0.25;

/// How long either sensor may be away from READY before follow mode gives up.
/// Longer than the geolocation retry delay, so a recoverable gap gets its retry
/// before this fires.
const SENSOR_GRACE_MS: f64 = 12000.0;

struct Follow {
    smoothed_azimuth: Option<f64>,
    was_following: bool,
    last_device: Option<PVF3>,
    last_heading: Option<f64>,
    /// When follow mode gives up, while a sensor is away from READY.
    grace_due: Option<f64>,
}

static FOLLOW: Global<Follow> = Global::new();

/// While follow mode is active, syncs the user's live position and heading
/// onto the map: follow mode's turn.
pub fn follow_mode_step() {
    let f = FOLLOW.get(|| Follow {
        smoothed_azimuth: None,
        was_following: false,
        last_device: None,
        last_heading: None,
        grace_due: None,
    });
    let camera = use_camera_store();
    let is_follow_mode = camera.is_follow_mode;
    let heading = use_device_orientation_store().orientation.and_then(|o| o.heading);
    let device_coord = use_map_navigation().device_coord;

    // Follow mode needs both sensors to mean anything: it's position (GPS)
    // fused with heading (compass). The tracking button only lets the user
    // enter follow mode once both are READY, and this re-checks it after
    // entry. Not immediately: either sensor can leave READY for a moment — a
    // GPS fix that misses its window indoors, an orientation gap — and dropping
    // follow mode on the first blip read as the mode switching itself off at
    // random. The exit still has to happen eventually: without it, follow mode
    // stays on with dead sensors, the camera freezes, and the disabled button
    // cannot be used to back out.
    let ready = use_device_location_store().status == DevicePoseStatus::Ready && use_device_orientation_store().status == DevicePoseStatus::Ready;
    if is_follow_mode && !ready {
        let due = *f.grace_due.get_or_insert(now() + SENSOR_GRACE_MS);
        if now() >= due {
            f.grace_due = None;
            camera.set_is_follow_mode(false);
        }
    } else {
        f.grace_due = None;
    }

    // dropping the smoothing anchor on exit means re-entering follow mode snaps
    // straight to the current heading instead of easing all the way from
    // wherever it was left off, possibly a while ago.
    let entered = is_follow_mode && !f.was_following;
    if !is_follow_mode {
        f.smoothed_azimuth = None;
    }
    f.was_following = is_follow_mode;
    if !is_follow_mode {
        return;
    }

    // 1. [Position sync] only when the GPS coordinate changes. No EMA here: the
    // companion's catch-up glide is the single smoothing stage. ground_or_raw
    // for the same reason pan and the recenter button use it: device_coord's
    // Z is raw GPS altitude, not baked terrain height.
    if let Some(device) = device_coord.filter(|d| entered || Some(*d) != f.last_device) {
        use_map_store().set_map_center(if use_settings_store().ground_snap { ground_or_raw(&device) } else { device });
    }
    f.last_device = device_coord;

    // 2. [Heading sync] only when the compass angle changes
    if let Some(heading) = heading.filter(|h| entered || Some(*h) != f.last_heading) {
        let raw_azimuth = heading * (core::f64::consts::PI / 180.0);
        let smoothed = match f.smoothed_azimuth {
            None => raw_azimuth,
            // shortest signed distance across the wrap (e.g. 359°→1° is a small
            // +2° step, not -358°), eased by HEADING_SMOOTH_ALPHA per sample.
            Some(s) => s + wrap(raw_azimuth - s, -core::f64::consts::PI, core::f64::consts::PI) * HEADING_SMOOTH_ALPHA,
        };
        f.smoothed_azimuth = Some(smoothed);
        camera.set_view_state(ViewPatch {
            azimuth: Some(smoothed),
            ..Default::default()
        });
    }
    f.last_heading = heading;
}
