// pxl-kernel/domain/control/sensor/reading.rs

//! The device's GPS and compass readings, and which changes are significant
//! enough to keep.

use crate::domain::control::cmd::Message;
pub use crate::domain::control::cmd::{PermissionAnswer, SensorStatus};
use xpute_guest::clock::now;
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceLocation {
    /// Longitude, latitude.
    pub coords: [f64; 2],
    /// Meters over sea level; none where the device gives none.
    pub alt: Option<f64>,
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

/// Where a sensor's reading is, as the snapshot carries it: its status, and
/// what failed where it failed (`err` is none but at `SensorStatus::Error`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SensorState {
    pub status: SensorStatus,
    pub err: Option<Message>,
}

impl SensorState {
    pub const fn of(status: SensorStatus) -> SensorState {
        SensorState { status, err: None }
    }

    pub const fn failed(err: Message) -> SensorState {
        SensorState {
            status: SensorStatus::Error,
            err: Some(err),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceLocationStore {
    pub is_tracking_location: bool,
    pub location: Option<DeviceLocation>,
    pub state: SensorState,
}

impl DeviceLocationStore {
    /// Updates only on a significant change; any fix says the watch delivers.
    pub fn set_location(
        &mut self,
        coords: [f64; 2],
        alt: Option<f64>,
        accuracy: f64,
        course: Option<f64>,
    ) {
        let timestamp = now();
        self.state = SensorState::of(SensorStatus::Ready);
        let Some(prev) = self.location else {
            self.location = Some(DeviceLocation {
                coords,
                alt,
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
        let lon_diff = (prev.coords[0] - coords[0]).abs();
        let lat_diff = (prev.coords[1] - coords[1]).abs();
        let is_significant_change =
            lat_diff > 0.00001 || lon_diff > 0.00001 || is_delta_over(prev.alt, alt, 0.5);

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
            alt: if is_significant_change { alt } else { prev.alt },
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
    pub state: SensorState,
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
            self.state = SensorState::of(SensorStatus::Ready);
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
