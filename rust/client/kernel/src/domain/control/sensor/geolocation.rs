// pxl-kernel/domain/control/sensor/geolocation.rs

//! The GPS sensor's decisions: when a watch runs and what an error means.
//! The watch itself is the shell's, and it outlives a momentary error: a
//! W3C watch ends only on PERMISSION_DENIED or clearWatch, and keeps trying
//! through POSITION_UNAVAILABLE and TIMEOUT
//! (https://www.w3.org/TR/geolocation/#watchposition-method), so nothing is
//! retried here; a device's module re-arms its own watch where its platform
//! ends one (ts/app/native/src/sensor.ts).

use crate::domain::control::cmd::GeolocationErrorCode;
use crate::domain::control::cmd::Message;
use crate::domain::control::sensor::reading::{device_location_store, SensorState, SensorStatus};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeolocationCommand {
    Start,
    Stop,
}

struct Geolocation {
    /// What a return to the foreground resumes.
    open: bool,
    /// The turn's last; the shell's watch follows it and ignores a repeat.
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
pub fn start_watching(supported: bool) {
    let store = device_location_store();
    if store.is_tracking_location {
        return;
    }
    if !supported {
        store.state = SensorState::failed(Message::GeolocationUnsupported);
        return;
    }
    store.state = SensorState::of(SensorStatus::Loading);
    store.set_is_tracking_location(true);
    geolocation().command = Some(GeolocationCommand::Start);
}

pub fn stop_watching() {
    let store = device_location_store();
    if store.is_tracking_location {
        store.set_is_tracking_location(false);
        geolocation().command = Some(GeolocationCommand::Stop);
    }
}

pub fn geolocation_open(supported: bool) {
    geolocation().open = true;
    start_watching(supported);
}

pub fn geolocation_close() {
    geolocation().open = false;
    stop_watching();
    device_location_store().state = SensorState::of(SensorStatus::Idle);
}

pub fn geolocation_visibility(visible: bool, supported: bool) {
    if !visible {
        stop_watching();
    } else if geolocation().open {
        start_watching(supported);
    }
}

/// A grant from outside the app recovers; otherwise a denial stuck until reload.
pub fn on_geolocation_permission_granted(supported: bool) {
    if !device_location_store().is_tracking_location {
        start_watching(supported);
    }
}

/// `altitude` is the ellipsoid's, as GPS gives it; kept over sea level, the
/// terrain's, by the geoid at the base of the leaf tile it falls in.
pub fn geolocation_fix(
    lng: f64,
    lat: f64,
    altitude: Option<f64>,
    accuracy: f64,
    course: Option<f64>,
) {
    let (x0, y0, _) = GROUND
        .cell_of(LEAF_ZOOM, &epsg4326::project(&[lng, lat, 0.0]))
        .tile()
        .rect();
    let [lng, lat, _] =
        epsg4326::from_mercator(&[epsg3857::mer_from_u(x0), epsg3857::mer_from_u(y0), 0.0]);
    let altitude = altitude.map(|h| h - epsg4326::geoid_m(lng, lat));
    device_location_store().set_location([lng, lat], altitude, accuracy, course);
}

/// A momentary error (unavailable, timed out) leaves the watch running, the
/// pose failed until its next fix; one that ended the watch (a denial, or a
/// device's watch that could not start) leaves nothing watching, which a
/// grant or a return to the foreground starts again.
pub fn geolocation_err(code: GeolocationErrorCode) {
    let store = device_location_store();
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
