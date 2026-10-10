// pxl-kernel/control/sensor/reading.rs

//! The device's GPS readings, and which changes are significant
//! enough to keep.

use pxl_spec::space::coord::adapter::epsg4326::G4326;

use crate::abi::cmd::Message;
pub use crate::abi::cmd::{DevicePoseStatus, PermissionAnswer};
use xpute_guest::clock::now;
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceLocation {
    /// Longitude, latitude, altitude.
    pub coords: G4326,
    /// Meters.
    pub accuracy: f64,
    /// Course over ground, 0-360 degrees.
    pub course: Option<f64>,
    pub timestamp: f64,
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
    /// Updates only on a significant change.
    pub fn set_location(&mut self, coords: G4326, accuracy: f64, course: Option<f64>) {
        let timestamp = now();
        let Some(prev) = self.location else {
            self.location = Some(DeviceLocation { coords, accuracy, course, timestamp });
            self.pose = Pose::Ready;
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
        let alt_diff = (prev.coords[2] - coords[2]).abs();
        let is_significant_change = lat_diff > 0.00001 || lng_diff > 0.00001 || alt_diff > 0.5;

        if !is_significant_change && prev.accuracy == accuracy && prev.course == course {
            return;
        }

        // Without movement the metadata still refreshes, so the UI has the freshest signal.
        self.location = Some(DeviceLocation {
            coords: if is_significant_change { coords } else { prev.coords },
            accuracy,
            course,
            timestamp,
        });
        self.pose = Pose::Ready;
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

static LOCATION: Global<DeviceLocationStore> = Global::new();

pub fn device_location_store() -> &'static mut DeviceLocationStore {
    LOCATION.get(DeviceLocationStore::default)
}
