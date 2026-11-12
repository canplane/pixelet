// pxl-core/domain/control/sensor/geolocation.rs

//! The GPS sensor's decisions: when a watch runs and what an error means.
//! The UI owns the watch itself, and says with each failure whether the watch
//! runs on (SENSOR_ERR `watching`): a W3C watch rides out all but
//! PERMISSION_DENIED (https://www.w3.org/TR/geolocation/#watchposition-method),
//! and a device's ends on any. Nothing is retried here. A watch that ended starts again on a
//! grant, a return to the foreground or a press of the location card.

use crate::abi::sensor::GeolocationErrorCode;
use crate::abi::ui::Message;
use crate::abi::ui::SensorStatus;
use crate::domain::control::sensor::platform;
use crate::domain::control::sensor::reading::{
    device_location_store, device_location_store_mut, SensorState,
};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::{Grid, GROUND};
use xpute_core::global::Global;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeolocationCommand {
    Start,
    Stop,
}

struct Geolocation {
    /// Whether the world's surface is open, so a return to the foreground
    /// starts the watch again.
    open: bool,
    /// The turn's last command. The UI's watch follows it and ignores a
    /// repeated command.
    cmd: Option<GeolocationCommand>,
}

static GEOLOCATION: Global<Geolocation> = Global::new();

fn geolocation() -> &'static mut Geolocation {
    GEOLOCATION.get(|| Geolocation {
        open: false,
        cmd: None,
    })
}

/// A watch runs while the store tracks the location
/// (`DeviceLocationStore::is_tracking_location`).
pub fn start_watching() {
    let store = device_location_store_mut();
    if store.is_tracking_location {
        return;
    }
    if !platform().geolocation {
        store.state = SensorState::failed(Message::GeolocationUnsupported);
        return;
    }
    // A watch started again after a hidden spell keeps the location it had
    // and reads as Loading until it gives a fix of its own.
    store.state = SensorState::of(SensorStatus::Loading);
    store.is_tracking_location = true;
    geolocation().cmd = Some(GeolocationCommand::Start);
}

/// The location is kept: a watch stopped while the app is hidden is not the
/// location gone. It is dropped where the location ends (`geolocation_close`,
/// `geolocation_err`).
pub fn stop_watching() {
    let store = device_location_store_mut();
    if store.is_tracking_location {
        store.is_tracking_location = false;
        geolocation().cmd = Some(GeolocationCommand::Stop);
    }
}

pub fn geolocation_open() {
    geolocation().open = true;
    start_watching();
}

pub fn geolocation_close() {
    geolocation().open = false;
    stop_watching();
    let store = device_location_store_mut();
    store.location = None;
    store.state = SensorState::of(SensorStatus::Idle);
}

pub fn geolocation_visibility(visible: bool) {
    if !visible {
        stop_watching();
    } else if geolocation().open {
        start_watching();
    }
}

/// A grant from outside the app, such as the device's settings, starts the
/// watch again. Without it, a denial would stick until the page reloads.
pub fn on_geolocation_permission_granted() {
    if !device_location_store().is_tracking_location {
        start_watching();
    }
}

/// A location fix. GPS gives `alt` over the ellipsoid. It is kept over sea
/// level, the datum the terrain uses, by the geoid's height at the base
/// corner of the leaf tile the fix falls in.
pub fn geolocation_fix(lon: f64, lat: f64, alt: Option<f64>, accuracy: f64, course: Option<f64>) {
    let (x0, y0, _) = GROUND
        .cell_of(LEAF_ZOOM, &epsg4326::project(&[lon, lat, 0.0]))
        .tile()
        .rect();
    let [base_lon, base_lat, _] =
        epsg4326::from_mercator(&[epsg3857::merc_from_u(x0), epsg3857::merc_from_u(y0), 0.0]);
    let alt = alt.map(|h| h - epsg4326::geoid_m(base_lon, base_lat));
    device_location_store_mut().set_location([lon, lat], alt, accuracy, course);
}

/// A location error. Whether the watch runs on past it (`watching`) is the
/// platform's to say. One it rides out leaves the watch running, and the location reads as
/// failed until the next fix. One that ended it leaves nothing watching and no
/// location, and a grant, a return to the foreground or a press starts the
/// watch again.
pub fn geolocation_err(code: GeolocationErrorCode, watching: bool) {
    let store = device_location_store_mut();
    store.state = SensorState::failed(match code {
        GeolocationErrorCode::PermissionDenied => Message::GeolocationPermissionDenied,
        GeolocationErrorCode::PositionUnavailable => Message::GeolocationPositionUnavailable,
        GeolocationErrorCode::Timeout => Message::GeolocationTimeout,
        GeolocationErrorCode::Other => Message::GeolocationGenericError,
    });
    if !watching {
        store.is_tracking_location = false;
        store.location = None;
    }
}

pub fn take_geolocation_cmd() -> Option<GeolocationCommand> {
    geolocation().cmd.take()
}

#[cfg(test)]
#[path = "geolocation.test.rs"]
mod test;
