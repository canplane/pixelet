// pxl-core/domain/control/sensor/geolocation.rs

//! The GPS sensor's decisions: when a watch runs and what an error means.
//! The UI owns the watch itself, and the watch outlives a momentary error. A
//! W3C watch ends only on PERMISSION_DENIED or clearWatch, and it keeps
//! trying through POSITION_UNAVAILABLE and TIMEOUT
//! (https://www.w3.org/TR/geolocation/#watchposition-method), so nothing is
//! retried here. A device's module re-arms its own watch where its platform
//! ends one (ts/client/src/service/platform/sensor.native.ts).

use crate::domain::cmd::GeolocationErrorCode;
use crate::domain::cmd::Message;
use crate::domain::cmd::SensorStatus;
use crate::domain::control::sensor::platform;
use crate::domain::control::sensor::reading::{
    device_location_store, device_location_store_mut, SensorState,
};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::layer::lattice::Grid;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;
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
    command: Option<GeolocationCommand>,
}

static GEOLOCATION: Global<Geolocation> = Global::new();

fn geolocation() -> &'static mut Geolocation {
    GEOLOCATION.get(|| Geolocation {
        open: false,
        command: None,
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
    store.state = SensorState::of(SensorStatus::Loading);
    store.set_is_tracking_location(true);
    geolocation().command = Some(GeolocationCommand::Start);
}

pub fn stop_watching() {
    let store = device_location_store_mut();
    if store.is_tracking_location {
        store.set_is_tracking_location(false);
        geolocation().command = Some(GeolocationCommand::Stop);
    }
}

pub fn geolocation_open() {
    geolocation().open = true;
    start_watching();
}

pub fn geolocation_close() {
    geolocation().open = false;
    stop_watching();
    device_location_store_mut().state = SensorState::of(SensorStatus::Idle);
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

/// A location error. A momentary error (unavailable, timed out) leaves the
/// watch running, and the location reads as failed until the next fix. An
/// error that ended the watch (a denial, or a device's watch that could not
/// start) leaves nothing watching, and a grant or a return to the foreground
/// starts the watch again.
pub fn geolocation_err(code: GeolocationErrorCode) {
    let store = device_location_store_mut();
    store.state = SensorState::failed(match code {
        GeolocationErrorCode::PermissionDenied => Message::GeolocationPermissionDenied,
        GeolocationErrorCode::PositionUnavailable => Message::GeolocationPositionUnavailable,
        GeolocationErrorCode::Timeout => Message::GeolocationTimeout,
        GeolocationErrorCode::Other => Message::GeolocationGenericError,
    });
    if matches!(
        code,
        GeolocationErrorCode::PermissionDenied | GeolocationErrorCode::Other
    ) {
        store.set_is_tracking_location(false);
    }
}

pub fn take_geolocation_command() -> Option<GeolocationCommand> {
    geolocation().command.take()
}

#[cfg(test)]
#[path = "geolocation.test.rs"]
mod test;
