// pxl-kernel/domain/control/sensor/reading.rs

//! The device's GPS and compass readings, and which changes are significant
//! enough to keep.

use crate::domain::control::cmd::Message;
pub use crate::domain::control::cmd::{DevicePoseStatus, PermissionAnswer};
use xpute_guest::clock::now;
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceLocation {
    /// Longitude, latitude.
    pub coords: [f64; 2],
    /// Meters over sea level; none where the device gives none.
    pub altitude: Option<f64>,
    /// Meters.
    pub accuracy: f64,
    /// Course over ground, 0-360 degrees.
    pub course: Option<f64>,
    pub timestamp: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceOrientation {
    /// 0-360 compass heading.
    pub heading: Option<f64>,
    /// -180-180 front/back tilt.
    pub beta: Option<f64>,
    /// -90-90 left/right tilt.
    pub gamma: Option<f64>,
    pub timestamp: f64,
}

fn is_delta_over(a: Option<f64>, b: Option<f64>, eps: f64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() > eps,
        _ => a != b,
    }
}

fn circular_delta(a: f64, b: f64, period: f64) -> f64 {
    let raw = (a - b).abs() % period;
    if raw > period / 2.0 {
        period - raw
    } else {
        raw
    }
}

/// A plain delta would see 359° vs 1° as 358° and let noise through around north.
fn is_heading_delta_over(a: Option<f64>, b: Option<f64>, eps: f64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => circular_delta(a, b, 360.0) > eps,
        _ => a != b,
    }
}

/// Where a sensor's reading is; a failure says what failed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pose {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed(Message),
    Unavailable,
}

impl Pose {
    pub fn status(self) -> DevicePoseStatus {
        match self {
            Pose::Idle => DevicePoseStatus::Idle,
            Pose::Loading => DevicePoseStatus::Loading,
            Pose::Ready => DevicePoseStatus::Ready,
            Pose::Failed(_) => DevicePoseStatus::Error,
            Pose::Unavailable => DevicePoseStatus::Unavailable,
        }
    }

    pub fn error(self) -> Option<Message> {
        match self {
            Pose::Failed(m) => Some(m),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceLocationStore {
    pub is_tracking_location: bool,
    pub location: Option<DeviceLocation>,
    pub pose: Pose,
}

impl DeviceLocationStore {
    /// Updates only on a significant change; any fix says the watch delivers.
    pub fn set_location(
        &mut self,
        coords: [f64; 2],
        altitude: Option<f64>,
        accuracy: f64,
        course: Option<f64>,
    ) {
        let timestamp = now();
        self.pose = Pose::Ready;
        let Some(prev) = self.location else {
            self.location = Some(DeviceLocation {
                coords,
                altitude,
                accuracy,
                course,
                timestamp,
            });
            return;
        };

        // Accepted under 100 m, or while accuracy improves.
        let is_improving = accuracy < prev.accuracy;
        if accuracy > 100.0 && !is_improving {
            return;
        }

        // Dead zone: ~1.1 m horizontally, 0.5 m in altitude.
        let lng_diff = (prev.coords[0] - coords[0]).abs();
        let lat_diff = (prev.coords[1] - coords[1]).abs();
        let is_significant_change =
            lat_diff > 0.00001 || lng_diff > 0.00001 || is_delta_over(prev.altitude, altitude, 0.5);

        if !is_significant_change && prev.accuracy == accuracy && prev.course == course {
            return;
        }

        // Without movement the metadata still refreshes, so the UI has the freshest signal.
        self.location = Some(DeviceLocation {
            coords: if is_significant_change {
                coords
            } else {
                prev.coords
            },
            altitude: if is_significant_change {
                altitude
            } else {
                prev.altitude
            },
            accuracy,
            course,
            timestamp,
        });
    }

    pub fn set_is_tracking_location(&mut self, enabled: bool) {
        if self.is_tracking_location == enabled {
            return;
        }
        self.is_tracking_location = enabled;
        if !enabled {
            self.location = None;
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceOrientationStore {
    pub is_tracking_orient: bool,
    pub orient: Option<DeviceOrientation>,
    pub pose: Pose,
}

impl DeviceOrientationStore {
    /// Dead-zone filtered: 1.5° of heading, 2° of tilt.
    pub fn set_orient(&mut self, heading: Option<f64>, beta: Option<f64>, gamma: Option<f64>) {
        let timestamp = now();
        let Some(prev) = self.orient else {
            self.orient = Some(DeviceOrientation {
                heading,
                beta,
                gamma,
                timestamp,
            });
            // Orientation alone counts as usable.
            self.pose = Pose::Ready;
            return;
        };

        let is_heading_changed = is_heading_delta_over(heading, prev.heading, 1.5);
        let is_tilt_changed =
            is_delta_over(beta, prev.beta, 2.0) || is_delta_over(gamma, prev.gamma, 2.0);
        if !is_heading_changed && !is_tilt_changed {
            return;
        }
        self.orient = Some(DeviceOrientation {
            heading,
            beta,
            gamma,
            timestamp,
        });
    }

    pub fn set_is_tracking_orient(&mut self, enabled: bool) {
        if self.is_tracking_orient == enabled {
            return;
        }
        self.is_tracking_orient = enabled;
        if !enabled {
            self.orient = None;
        }
    }
}

static LOCATION: Global<DeviceLocationStore> = Global::new();
static ORIENT: Global<DeviceOrientationStore> = Global::new();

pub fn device_location_store() -> &'static mut DeviceLocationStore {
    LOCATION.get(DeviceLocationStore::default)
}

pub fn device_orient_store() -> &'static mut DeviceOrientationStore {
    ORIENT.get(DeviceOrientationStore::default)
}
