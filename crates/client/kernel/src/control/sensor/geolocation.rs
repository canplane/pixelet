// pxl-kernel/control/sensor/geolocation.rs

//! The GPS sensor's decisions: when a watch runs, what an error means, and
//! when to retry. The watch itself is the shell's.

pub use crate::abi::cmd::GeolocationErrorCode;
use crate::abi::cmd::Message;
use crate::control::sensor::store::{device_location_store, Pose};
use crate::world::change::wake_at;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

/// A timeout or unavailable fix is momentary (indoors, tunnels). The watch
/// must stop on error, or a dead one races the next, but nothing else
/// restarted it: one bad minute underground ended tracking until the app was
/// reopened. One pending retry, cleared by a stop.
const GEOLOCATION_RETRY_MS: f64 = 5000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeolocationCommand {
    Start,
    Stop,
}

struct Geolocation {
    /// What a return to the foreground resumes.
    open: bool,
    watching: bool,
    retry_at: Option<f64>,
    commands: Vec<GeolocationCommand>,
}

static GEOLOCATION: Global<Geolocation> = Global::new();

fn geolocation() -> &'static mut Geolocation {
    GEOLOCATION.get(|| Geolocation {
        open: false,
        watching: false,
        retry_at: None,
        commands: Vec::new(),
    })
}

pub fn start_watching(supported: bool) {
    let g = geolocation();
    g.retry_at = None;
    if g.watching {
        return;
    }
    let store = device_location_store();
    if !supported {
        store.pose = Pose::Failed(Message::GeolocationUnsupported);
        return;
    }
    store.pose = Pose::Loading;
    store.set_is_tracking_location(true);
    g.watching = true;
    g.commands.push(GeolocationCommand::Start);
}

pub fn stop_watching() {
    let g = geolocation();
    g.retry_at = None;
    if g.watching {
        g.watching = false;
        g.commands.push(GeolocationCommand::Stop);
        device_location_store().set_is_tracking_location(false);
    }
}

pub fn geolocation_open(supported: bool) {
    geolocation().open = true;
    start_watching(supported);
}

pub fn geolocation_close() {
    geolocation().open = false;
    stop_watching();
    device_location_store().pose = Pose::Idle;
}

pub fn geolocation_visibility(visible: bool, supported: bool) {
    if !visible {
        stop_watching();
    } else if geolocation().open {
        start_watching(supported);
    }
}

/// A grant from outside the app recovers; otherwise a denial stuck until reload.
pub fn geolocation_permission_granted(supported: bool) {
    if !device_location_store().is_tracking_location {
        start_watching(supported);
    }
}

pub fn geolocation_fix(longitude: f64, latitude: f64, altitude: Option<f64>, accuracy: f64, course: Option<f64>) {
    device_location_store().set_location([longitude, latitude, altitude.unwrap_or(0.0)], accuracy, course);
}

/// The shell stops the watch before this: a dead watch keeps calling back
/// and a later start would race it.
pub fn geolocation_error(code: GeolocationErrorCode) {
    let g = geolocation();
    g.watching = false;
    let store = device_location_store();
    store.set_is_tracking_location(false);
    store.pose = Pose::Failed(match code {
        GeolocationErrorCode::PermissionDenied => Message::GeolocationPermissionDenied,
        GeolocationErrorCode::PositionUnavailable => Message::GeolocationPositionUnavailable,
        GeolocationErrorCode::Timeout => Message::GeolocationTimeout,
        GeolocationErrorCode::Other => Message::GeolocationGenericError,
    });
    // Denial is the user's to undo; anything else is retried.
    g.retry_at = (code != GeolocationErrorCode::PermissionDenied).then(|| now() + GEOLOCATION_RETRY_MS);
}

pub fn geolocation_step(supported: bool) {
    match geolocation().retry_at {
        Some(at) if now() >= at => start_watching(supported),
        Some(at) => wake_at(at),
        None => {}
    }
}

pub fn take_geolocation_commands() -> Vec<GeolocationCommand> {
    core::mem::take(&mut geolocation().commands)
}
