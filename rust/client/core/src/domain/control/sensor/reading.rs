// pxl-core/domain/control/sensor/reading.rs

//! The device's GPS and compass readings, and which changes are significant
//! enough to keep.

use crate::abi::overlay::Message;
use crate::abi::overlay::SensorStatus;
use pxl_global::Global;
use xpute_kit::math::scalar::wrap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DeviceLocation {
    /// Longitude and latitude, in degrees.
    pub lon_lat: [f64; 2],
    /// Meters above sea level. None where the device gives no altitude.
    pub alt: Option<f64>,
    /// The fix's accuracy, in meters.
    pub accuracy: f64,
    /// Course over ground, 0-360 degrees.
    pub course: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DeviceOrientation {
    /// Compass heading, 0-360 degrees, or none where the reading gave none.
    pub heading: Option<f64>,
}

/// Whether `a` lies more than `eps` from `b` by `dist`; a value that appears or
/// goes is always a change.
fn is_delta_over(a: Option<f64>, b: Option<f64>, eps: f64, dist: fn(f64, f64) -> f64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => dist(a, b) > eps,
        _ => a != b,
    }
}

fn plain_delta(a: f64, b: f64) -> f64 {
    (a - b).abs()
}

/// How far apart two headings lie on the circle: 359° and 1° are 2° apart,
/// where a plain difference would make them 358°.
fn heading_delta(a: f64, b: f64) -> f64 {
    wrap(a - b, -180.0, 180.0).abs()
}

/// The location's state as the snapshot carries it: its status, and what
/// failed when it failed. `err` is None except at `SensorStatus::Error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SensorState {
    pub status: SensorStatus,
    pub err: Option<Message>,
}

/// A sensor is idle until it is asked to start.
impl Default for SensorState {
    fn default() -> SensorState {
        SensorState::of(SensorStatus::Idle)
    }
}

impl SensorState {
    pub(crate) const fn of(status: SensorStatus) -> SensorState {
        SensorState { status, err: None }
    }

    pub(crate) const fn failed(err: Message) -> SensorState {
        SensorState {
            status: SensorStatus::Error,
            err: Some(err),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DeviceLocationStore {
    pub is_tracking_location: bool,
    pub location: Option<DeviceLocation>,
    pub state: SensorState,
}

impl DeviceLocationStore {
    /// Takes a fix, and says whether the position changed. The location changes
    /// only on a significant change, but any fix marks the sensor Ready, since it
    /// shows the watch delivers.
    pub(crate) fn set_location(
        &mut self,
        lon_lat: [f64; 2],
        alt: Option<f64>,
        accuracy: f64,
        course: Option<f64>,
    ) -> bool {
        self.state = SensorState::of(SensorStatus::Ready);
        let Some(prev) = self.location else {
            self.location = Some(DeviceLocation {
                lon_lat,
                alt,
                accuracy,
                course,
            });
            return true;
        };

        // A fix is accepted when its accuracy is within 100 m, or when the
        // accuracy improves.
        let is_improving = accuracy < prev.accuracy;
        if accuracy > 100.0 && !is_improving {
            return false;
        }

        // The dead zone is about 1.1 m horizontally and 0.5 m in altitude.
        let lon_diff = (prev.lon_lat[0] - lon_lat[0]).abs();
        let lat_diff = (prev.lon_lat[1] - lon_lat[1]).abs();
        let is_significant_change = lat_diff > 0.00001
            || lon_diff > 0.00001
            || is_delta_over(prev.alt, alt, 0.5, plain_delta);

        if !is_significant_change && prev.accuracy == accuracy && prev.course == course {
            return false;
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
        });
        is_significant_change
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeviceOrientationStore {
    pub is_tracking_orient: bool,
    pub orient: Option<DeviceOrientation>,
    pub status: SensorStatus,
}

/// The device's orientation is idle until it is asked to start.
impl Default for DeviceOrientationStore {
    fn default() -> DeviceOrientationStore {
        DeviceOrientationStore {
            is_tracking_orient: false,
            orient: None,
            status: SensorStatus::Idle,
        }
    }
}

impl DeviceOrientationStore {
    /// Takes a reading, ignoring a change of heading within a dead zone of
    /// 1.5°, and says whether the heading it holds changed. The first reading
    /// makes the sensor Ready, even one without a heading.
    pub(crate) fn set_orient(&mut self, heading: Option<f64>) -> bool {
        let Some(prev) = self.orient else {
            self.orient = Some(DeviceOrientation { heading });
            self.status = SensorStatus::Ready;
            return heading.is_some();
        };
        let changed = is_delta_over(heading, prev.heading, 1.5, heading_delta);
        if changed {
            self.orient = Some(DeviceOrientation { heading });
        }
        changed
    }

    pub(crate) fn set_is_tracking_orient(&mut self, enabled: bool) {
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

pub(crate) fn device_location_store() -> &'static DeviceLocationStore {
    device_location_store_mut()
}

pub(crate) fn device_location_store_mut() -> &'static mut DeviceLocationStore {
    LOCATION.get(DeviceLocationStore::default)
}

pub(crate) fn device_orient_store() -> &'static DeviceOrientationStore {
    device_orient_store_mut()
}

pub(crate) fn device_orient_store_mut() -> &'static mut DeviceOrientationStore {
    ORIENT.get(DeviceOrientationStore::default)
}

#[cfg(test)]
#[path = "reading.test.rs"]
mod test;
