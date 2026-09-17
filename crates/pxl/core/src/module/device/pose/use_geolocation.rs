// pxl_core/module/device/pose/use_geolocation.rs

//! The device's GPS sensor, as decisions: when a watch should run, what an
//! error means, and when to try again. The watch itself is the shell's — it
//! takes `take_geolocation_commands` and reports fixes and errors back.

use pxl_shared::spec::message::Message;

use super::store::{use_device_location_store, DevicePoseStatus};
use crate::sys::global::{now, Global};

/// A TIMEOUT or POSITION_UNAVAILABLE is a report about this moment, not about
/// the session: indoors, in a tunnel, between buildings, a fix simply does not
/// arrive for a while. The error path has to stop the watch — a dead watch left
/// subscribed is what once had two of them racing and flickering device_coord
/// null/non-null — but that left nothing running afterwards either, and the
/// only things that ever restart it are a permission change or a return to the
/// foreground. So one bad minute underground ended location tracking until the
/// app was backgrounded and reopened, taking follow mode with it.
///
/// Retried instead, once, after a pause. One pending retry, cleared by a stop,
/// so this cannot become the second racing watch.
const GEOLOCATION_RETRY_MS: f64 = 5000.0;

/// What the shell's watch should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeolocationCommand {
    /// Start watching: high accuracy, a fix at most MAXIMUM_AGE_MS old, and a
    /// TIMEOUT_MS timeout.
    Start,
    Stop,
}

/// A brand-new high-accuracy fix within ten seconds is more than a phone indoors
/// routinely produces; a five-second-old fix is indistinguishable at walking
/// pace, and the longer 30s timeout rides out a gap instead of declaring
/// failure in the middle of one.
pub const MAXIMUM_AGE_MS: f64 = 5000.0;
pub const TIMEOUT_MS: f64 = 30000.0;

/// The browser's PositionError codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeolocationErrorCode {
    PermissionDenied,
    PositionUnavailable,
    Timeout,
    Other,
}

struct Geolocation {
    subscribers: u32,
    watching: bool,
    retry_at: Option<f64>,
    commands: Vec<GeolocationCommand>,
}

static GEOLOCATION: Global<Geolocation> = Global::new();

fn geolocation() -> &'static mut Geolocation {
    GEOLOCATION.get(|| Geolocation {
        subscribers: 0,
        watching: false,
        retry_at: None,
        commands: Vec::new(),
    })
}

/// Starts sensor reception, guarded against duplicate activation.
pub fn start_watching(supported: bool) {
    let g = geolocation();
    g.retry_at = None;
    if g.watching {
        return;
    }
    let store = use_device_location_store();
    if !supported {
        store.set_error(Message::GeolocationUnsupported);
        return;
    }
    store.set_status(DevicePoseStatus::Loading);
    store.set_is_tracking_location(true);
    g.watching = true;
    g.commands.push(GeolocationCommand::Start);
}

/// Stops sensor reception and releases resources.
pub fn stop_watching() {
    let g = geolocation();
    g.retry_at = None;
    if g.watching {
        g.watching = false;
        g.commands.push(GeolocationCommand::Stop);
        use_device_location_store().set_is_tracking_location(false);
    }
}

/// A subscriber arrived; the first starts the sensor.
pub fn geolocation_subscribe(supported: bool) {
    let g = geolocation();
    g.subscribers += 1;
    if g.subscribers == 1 {
        start_watching(supported);
    }
}

/// A subscriber left; the last stops the sensor.
pub fn geolocation_unsubscribe() {
    let g = geolocation();
    g.subscribers = g.subscribers.saturating_sub(1);
    if g.subscribers == 0 {
        stop_watching();
        let store = use_device_location_store();
        if store.status != DevicePoseStatus::Idle {
            store.set_status(DevicePoseStatus::Idle);
        }
    }
}

/// The tab's visibility: paused in the background to save battery, resumed on
/// returning to the foreground.
pub fn geolocation_visibility(visible: bool, supported: bool) {
    if !visible {
        stop_watching();
    } else if geolocation().subscribers > 0 {
        start_watching(supported);
    }
}

/// Recovers from a denied/error state the moment the user grants location
/// access from outside the app — without this a denial leaves the store stuck
/// in ERROR until the page reloads.
pub fn geolocation_permission_granted(supported: bool) {
    if !use_device_location_store().is_tracking_location {
        start_watching(supported);
    }
}

/// A fix from the watch. The adapter normalizes across browsers: lat/lng are
/// required, altitude can be absent, accuracy and heading are guarded too.
pub fn geolocation_fix(longitude: f64, latitude: f64, altitude: Option<f64>, accuracy: f64, heading: Option<f64>) {
    use_device_location_store().set_location([longitude, latitude, altitude.unwrap_or(0.0)], accuracy, heading);
}

/// An error from the watch. The watch is stopped by the shell before this — a
/// dead watch keeps calling back, and a later start would race it.
pub fn geolocation_error(code: GeolocationErrorCode) {
    let g = geolocation();
    g.watching = false;
    let store = use_device_location_store();
    store.set_is_tracking_location(false);
    store.set_error(match code {
        // Actionable, not just descriptive: the one case the user can fix.
        GeolocationErrorCode::PermissionDenied => Message::GeolocationPermissionDenied,
        GeolocationErrorCode::PositionUnavailable => Message::GeolocationPositionUnavailable,
        GeolocationErrorCode::Timeout => Message::GeolocationTimeout,
        GeolocationErrorCode::Other => Message::GeolocationGenericError,
    });
    // Permission denial is the user's to undo; anything else is worth another
    // try, since the sensor itself is presumed fine.
    g.retry_at = (code != GeolocationErrorCode::PermissionDenied).then(|| now() + GEOLOCATION_RETRY_MS);
}

/// The sensor's turn: a due retry starts the watch again.
pub fn geolocation_step(supported: bool) {
    if geolocation().retry_at.is_some_and(|at| now() >= at) {
        start_watching(supported);
    }
}

/// What the shell's watch should do, in order.
pub fn take_geolocation_commands() -> Vec<GeolocationCommand> {
    core::mem::take(&mut geolocation().commands)
}
