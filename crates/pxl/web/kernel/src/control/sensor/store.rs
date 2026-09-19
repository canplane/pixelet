// pxl-kernel/control/sensor/store.rs

//! The device's physical sensors (GPS, compass) — data and reception state.
//! The sensors themselves are the shell's; what they report lands here, and
//! what is significant enough to keep is decided here.

use pxl_spec::abi::message::Message;
use pxl_spec::space::adapter::epsg4326::G4326;

use crate::global::Global;
use crate::host::now;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DevicePoseStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceLocation {
    /// Current longitude/latitude/altitude.
    pub coords: G4326,
    /// GPS accuracy, in meters.
    pub accuracy: f64,
    /// Heading of travel from GPS course, 0-360 degrees.
    pub course: Option<f64>,
    pub timestamp: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceOrientation {
    /// 0-360: compass heading (Z rotation).
    pub heading: Option<f64>,
    /// -180-180: front/back tilt (X rotation).
    pub beta: Option<f64>,
    /// -90-90: left/right tilt (Y rotation).
    pub gamma: Option<f64>,
    pub timestamp: f64,
}

fn is_delta_over(a: Option<f64>, b: Option<f64>, eps: f64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() > eps,
        _ => a != b,
    }
}

/// Shortest distance between two values on a circle of the given period (e.g.
/// 360 for compass degrees) — unlike a plain subtraction, this stays small
/// across the wrap (359 vs 1 is a 2-unit gap, not 358).
fn circular_delta(a: f64, b: f64, period: f64) -> f64 {
    let raw = (a - b).abs() % period;
    if raw > period / 2.0 {
        period - raw
    } else {
        raw
    }
}

/// Same dead-zone check as is_delta_over, but for a circular (wrapping) value
/// like compass heading. Plain is_delta_over would see 359° vs 1° as a 358°
/// jump and let every bit of sensor noise through right around due north.
fn is_heading_delta_over(a: Option<f64>, b: Option<f64>, eps: f64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => circular_delta(a, b, 360.0) > eps,
        _ => a != b,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceLocationStore {
    /// Whether location tracking is active.
    pub is_tracking_location: bool,
    pub location: Option<DeviceLocation>,
    /// Overall location-service reception status.
    pub status: DevicePoseStatus,
    /// What went wrong; the shell turns it into the words it shows.
    pub error: Option<Message>,
}

impl DeviceLocationStore {
    /// Validates raw GPS data from hardware, and only updates state on a
    /// significant change.
    pub fn set_location(&mut self, coords: G4326, accuracy: f64, course: Option<f64>) {
        let timestamp = now();
        let Some(prev) = self.location else {
            // 1. First-fix pass
            self.location = Some(DeviceLocation { coords, accuracy, course, timestamp });
            self.status = DevicePoseStatus::Ready;
            self.error = None;
            return;
        };

        // 2. Accuracy guard (accept if < 100m, or if accuracy is improving)
        let is_improving = accuracy < prev.accuracy;
        if accuracy > 100.0 && !is_improving {
            return;
        }

        // 3. Dead-zone filtering (~1.1m for lat/lng, 0.5m for altitude)
        let lng_diff = (prev.coords[0] - coords[0]).abs();
        let lat_diff = (prev.coords[1] - coords[1]).abs();
        let alt_diff = (prev.coords[2] - coords[2]).abs();
        let is_significant_change = lat_diff > 0.00001 || lng_diff > 0.00001 || alt_diff > 0.5;

        if !is_significant_change && prev.accuracy == accuracy && prev.course == course {
            return;
        }

        // 4. Update strategy: even without significant movement, refresh
        // metadata (accuracy/course/timestamp) so the UI has the freshest
        // signal info.
        self.location = Some(DeviceLocation {
            coords: if is_significant_change { coords } else { prev.coords },
            accuracy,
            course,
            timestamp,
        });
        self.status = DevicePoseStatus::Ready;
        self.error = None;
    }

    pub fn set_is_tracking_location(&mut self, enabled: bool) {
        if self.is_tracking_location == enabled {
            return;
        }
        self.is_tracking_location = enabled;
        // reset location on stop, to avoid stale data
        if !enabled {
            self.location = None;
        }
    }

    pub fn set_error(&mut self, error: Message) {
        if self.status == DevicePoseStatus::Error && self.error == Some(error) {
            return;
        }
        self.status = DevicePoseStatus::Error;
        self.error = Some(error);
    }
}

/// A sensor permission, as SENSOR_PERMISSION carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionAnswer {
    /// The shell's prompt is up.
    Asking,
    Granted,
    Denied,
    /// The prompt itself threw.
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceOrientationStore {
    /// Whether the compass listener is registered and running.
    pub is_tracking_orientation: bool,
    pub orientation: Option<DeviceOrientation>,
    pub status: DevicePoseStatus,
    /// What went wrong; the shell turns it into the words it shows.
    pub error: Option<Message>,
}

impl DeviceOrientationStore {
    /// Updates compass sensor data (dead-zone filtered).
    pub fn set_orientation(&mut self, heading: Option<f64>, beta: Option<f64>, gamma: Option<f64>) {
        let timestamp = now();
        let Some(prev) = self.orientation else {
            self.orientation = Some(DeviceOrientation { heading, beta, gamma, timestamp });
            // orientation data alone is enough to count as usable
            self.status = DevicePoseStatus::Ready;
            self.error = None;
            return;
        };

        // dead-zone filter, keyed on heading (the most important field): 1.5°
        // change, using the circular-aware delta since heading wraps
        let is_heading_changed = is_heading_delta_over(heading, prev.heading, 1.5);
        // tilt (beta/gamma) change check, ~2° — avoids visible jitter
        let is_tilt_changed = is_delta_over(beta, prev.beta, 2.0) || is_delta_over(gamma, prev.gamma, 2.0);
        if !is_heading_changed && !is_tilt_changed {
            return;
        }
        self.orientation = Some(DeviceOrientation { heading, beta, gamma, timestamp });
    }

    pub fn set_is_tracking_orientation(&mut self, enabled: bool) {
        if self.is_tracking_orientation == enabled {
            return;
        }
        self.is_tracking_orientation = enabled;
        if !enabled {
            self.orientation = None;
        }
    }

    pub fn set_error(&mut self, error: Message) {
        if self.status == DevicePoseStatus::Error && self.error == Some(error) {
            return;
        }
        self.status = DevicePoseStatus::Error;
        self.error = Some(error);
    }
}

static LOCATION: Global<DeviceLocationStore> = Global::new();
static ORIENTATION: Global<DeviceOrientationStore> = Global::new();

pub fn device_location_store() -> &'static mut DeviceLocationStore {
    LOCATION.get(DeviceLocationStore::default)
}

pub fn device_orientation_store() -> &'static mut DeviceOrientationStore {
    ORIENTATION.get(DeviceOrientationStore::default)
}
