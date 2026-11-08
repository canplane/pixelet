// pxl-core/domain/control/sensor/device_orientation.rs

//! The compass sensor's decisions: the heading an event means, when to
//! listen, and when silence counts as a lost sensor. The UI owns the
//! listener and iOS's permission prompt, and the core hears only the answer.

use crate::domain::cmd::Message;

use crate::domain::cmd::{PermissionAnswer, SensorStatus};
use crate::domain::control::sensor::platform;
use crate::domain::control::sensor::reading::{device_orient_store, SensorState};
use crate::sys::sched::edge::wake_at;
use xpute_core::clock::now;
use xpute_core::global::Global;

/// Orientation has no Permissions API, so a revoked permission is never
/// reported. A gap this long while tracking is taken as a loss instead. This
/// holds only where the sensor talks while still (`silence_is_loss`).
const WATCHDOG_TIMEOUT_MS: f64 = 8000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrientationCommand {
    /// The UI listens to deviceorientationabsolute where the browser has it.
    Listen,
    Remove,
}

#[derive(Clone, Copy, Default)]
pub struct OrientationPlatform {
    pub supported: bool,
    /// On iOS 13 and later, requestPermission must run inside a user
    /// gesture.
    pub needs_permission: bool,
    /// Whether silence means the sensor is lost. A browser's sensor talks
    /// even while the device is still. A device's sensor (Core Location)
    /// speaks only on change and reports its own failure.
    pub silence_is_loss: bool,
}

struct Orientation {
    /// The time the watchdog measures silence from: the last event, the
    /// start of tracking, or the page's return to view. It is read only while
    /// tracking, and every start of tracking sets it.
    last_event_at: f64,
    /// The turn's last command. The UI's listener follows it and ignores a
    /// repeated command.
    command: Option<OrientationCommand>,
}

static ORIENT: Global<Orientation> = Global::new();

fn orient() -> &'static mut Orientation {
    ORIENT.get(|| Orientation {
        last_event_at: 0.0,
        command: None,
    })
}

/// An orientation event. iOS's compass heading is preferred, and otherwise
/// the heading is derived from alpha.
pub fn orient_event(
    webkit_compass_heading: Option<f64>,
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) {
    // An API without a sensor answers once with nothing and never again, so
    // the sensor counts as unavailable.
    if webkit_compass_heading.is_none() && alpha.is_none() && beta.is_none() && gamma.is_none() {
        orient().command = Some(OrientationCommand::Remove);
        let store = device_orient_store();
        store.set_is_tracking_orient(false);
        store.state = SensorState::of(SensorStatus::Unavailable);
        return;
    }
    orient().last_event_at = now();
    let heading = webkit_compass_heading.or_else(|| alpha.map(|a| (360.0 - a) % 360.0));
    device_orient_store().set_orient(heading, beta, gamma);
}

/// Asks for the heading. Returns true when the heading is tracked or now
/// asked for. Behind a permission, only the UI's prompt can start it, so
/// this returns false there.
pub fn request_orient() -> bool {
    let store = device_orient_store();
    if store.is_tracking_orient {
        return true;
    }
    if !platform().orient.supported || store.state.status == SensorStatus::Unavailable {
        store.state = SensorState::of(SensorStatus::Unavailable);
        return false;
    }
    if platform().orient.needs_permission {
        return false;
    }
    store.state = SensorState::of(SensorStatus::Loading);
    let o = orient();
    o.command = Some(OrientationCommand::Listen);
    o.last_event_at = now();
    store.set_is_tracking_orient(true);
    true
}

/// The answer to the orientation permission. Where no prompt gates the
/// sensor, a grant came from outside the app (the device's settings) and
/// starts the listener here.
pub fn orient_permission(answer: PermissionAnswer) {
    let store = device_orient_store();
    match answer {
        PermissionAnswer::Asking => {
            if !store.is_tracking_orient {
                store.state = SensorState::of(SensorStatus::Loading);
            }
        }
        PermissionAnswer::Granted if platform().orient.needs_permission => {
            orient().last_event_at = now();
            store.set_is_tracking_orient(true);
        }
        PermissionAnswer::Granted => {
            request_orient();
        }
        PermissionAnswer::Denied => {
            store.set_is_tracking_orient(false);
            store.state = SensorState::failed(Message::OrientationPermissionDenied);
        }
        PermissionAnswer::Failed => {
            store.set_is_tracking_orient(false);
            store.state = SensorState::failed(Message::OrientationPermissionError);
        }
    }
}

pub fn on_orient_lost() {
    let store = device_orient_store();
    store.set_is_tracking_orient(false);
    store.state = SensorState::failed(Message::OrientationSignalLost);
}

pub fn orient_close() {
    orient().command = Some(OrientationCommand::Remove);
    let store = device_orient_store();
    store.set_is_tracking_orient(false);
    store.state = SensorState::of(SensorStatus::Idle);
}

/// Called when the page comes back into view. Background tabs stop
/// dispatching events, so judging the gap on return would mistake a hidden
/// page for a lost sensor. The watchdog measures afresh from now instead.
pub fn orient_visible() {
    orient().last_event_at = now();
}

/// Judges whether the heading is lost. It is armed only while a heading is
/// tracked on a platform where silence is the only sign of loss, so nothing
/// is woken while nothing is tracked. It wakes once, when the silence since
/// the last event would amount to a loss. An event before then moves that
/// deadline, so a sensor that keeps talking is never polled. A heading is
/// asked for when the sensors open and again when the user follows
/// (navigation/follow_mode.rs `tracking_action`).
pub fn orient_watchdog_step() {
    if !platform().orient.silence_is_loss || !device_orient_store().is_tracking_orient {
        return;
    }
    let o = orient();
    let lost_at = o.last_event_at + WATCHDOG_TIMEOUT_MS;
    if now() < lost_at {
        wake_at(lost_at);
        return;
    }
    o.command = Some(OrientationCommand::Remove);
    on_orient_lost();
}

pub fn take_orient_command() -> Option<OrientationCommand> {
    orient().command.take()
}
