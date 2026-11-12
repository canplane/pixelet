// pxl-core/domain/control/sensor/reading.rs

//! The device's GPS and compass readings, and which changes are significant
//! enough to keep.

use crate::abi::ui::Message;
use crate::abi::ui::SensorStatus;
use xpute_core::clock::now;
use xpute_core::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceLocation {
    /// Longitude and latitude, in degrees.
    pub lon_lat: [f64; 2],
    /// Meters above sea level. None where the device gives no altitude.
    pub alt: Option<f64>,
    /// The fix's accuracy, in meters.
    pub accuracy: f64,
    /// Course over ground, 0-360 degrees.
    pub course: Option<f64>,
    pub timestamp: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceOrientation {
    /// Compass heading, 0-360 degrees.
    pub heading: Option<f64>,
    /// Front-to-back tilt, -180 to 180 degrees.
    pub beta: Option<f64>,
    /// Left-to-right tilt, -90 to 90 degrees.
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

/// The location's state as the snapshot carries it: its status, and what
/// failed when it failed. `err` is None except at `SensorStatus::Error`.
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
    /// Takes a fix. The location changes only on a significant change, but
    /// any fix marks the sensor Ready, since it shows the watch delivers.
    pub fn set_location(
        &mut self,
        lon_lat: [f64; 2],
        alt: Option<f64>,
        accuracy: f64,
        course: Option<f64>,
    ) {
        let timestamp = now();
        self.state = SensorState::of(SensorStatus::Ready);
        let Some(prev) = self.location else {
            self.location = Some(DeviceLocation {
                lon_lat,
                alt,
                accuracy,
                course,
                timestamp,
            });
            return;
        };

        // A fix is accepted when its accuracy is within 100 m, or when the
        // accuracy improves.
        let is_improving = accuracy < prev.accuracy;
        if accuracy > 100.0 && !is_improving {
            return;
        }

        // The dead zone is about 1.1 m horizontally and 0.5 m in altitude.
        let lon_diff = (prev.lon_lat[0] - lon_lat[0]).abs();
        let lat_diff = (prev.lon_lat[1] - lon_lat[1]).abs();
        let is_significant_change =
            lat_diff > 0.00001 || lon_diff > 0.00001 || is_delta_over(prev.alt, alt, 0.5);

        if !is_significant_change && prev.accuracy == accuracy && prev.course == course {
            return;
        }

        // Without movement, the accuracy and course still refresh, so the UI
        // shows the freshest signal.
        self.location = Some(DeviceLocation {
            lon_lat: if is_significant_change {
                lon_lat
            } else {
                prev.lon_lat
            },
            alt: if is_significant_change { alt } else { prev.alt },
            accuracy,
            course,
            timestamp,
        });
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceOrientationStore {
    pub is_tracking_orient: bool,
    pub orient: Option<DeviceOrientation>,
    pub status: SensorStatus,
}

impl DeviceOrientationStore {
    /// Takes an orientation reading, ignoring changes within a dead zone of
    /// 1.5° of heading and 2° of tilt.
    pub fn set_orient(&mut self, heading: Option<f64>, beta: Option<f64>, gamma: Option<f64>) {
        let timestamp = now();
        let Some(prev) = self.orient else {
            self.orient = Some(DeviceOrientation {
                heading,
                beta,
                gamma,
                timestamp,
            });
            // Any first reading makes the sensor Ready, even one without a
            // heading.
            self.status = SensorStatus::Ready;
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

pub fn device_location_store() -> &'static DeviceLocationStore {
    device_location_store_mut()
}

pub fn device_location_store_mut() -> &'static mut DeviceLocationStore {
    LOCATION.get(DeviceLocationStore::default)
}

pub fn device_orient_store() -> &'static DeviceOrientationStore {
    device_orient_store_mut()
}

pub fn device_orient_store_mut() -> &'static mut DeviceOrientationStore {
    ORIENT.get(DeviceOrientationStore::default)
}
