// pxl-kernel/domain/control/sensor/device_orientation.rs

//! The compass sensor's decisions: the heading an event means, when to
//! listen, and when silence counts as lost. The listener and iOS's permission
//! prompt are the shell's; the kernel hears only the answer.

use crate::domain::control::cmd::Message;

use crate::domain::control::sensor::reading::{device_orientation_store, PermissionAnswer, Pose};
use crate::sys::sched::edge::wake_at;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

/// Orientation has no Permissions API, so revocation cannot be pushed; a long
/// gap while tracking is taken as loss. Only valid where the sensor talks
/// while still (`silence_is_loss`).
const WATCHDOG_TIMEOUT_MS: f64 = 8000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrientationCommand {
    /// Uses deviceorientationabsolute where the browser has it.
    Listen,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrientationPlatform {
    pub supported: bool,
    /// iOS 13+: requestPermission must run inside a user gesture.
    pub needs_permission: bool,
    /// A browser's sensor talks while still; a device's (Core Location) speaks
    /// only on change and reports failure itself.
    pub silence_is_loss: bool,
}

struct Orientation {
    /// What the watchdog judges silence from: the last event, or where
    /// tracking began or the page came back. Read only while tracking, and
    /// set wherever tracking begins.
    last_event_at: f64,
    /// The turn's last; the shell's listener follows it and ignores a repeat.
    command: Option<OrientationCommand>,
}

static ORIENTATION: Global<Orientation> = Global::new();

fn orientation() -> &'static mut Orientation {
    ORIENTATION.get(|| Orientation {
        last_event_at: 0.0,
        command: None,
    })
}

/// iOS's compass heading preferred, the alpha-derived one otherwise.
pub fn orientation_event(
    webkit_compass_heading: Option<f64>,
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) {
    // An API with no sensor answers once, empty, and never again.
    if webkit_compass_heading.is_none() && alpha.is_none() && beta.is_none() && gamma.is_none() {
        orientation().command = Some(OrientationCommand::Remove);
        let store = device_orientation_store();
        store.set_is_tracking_orientation(false);
        store.pose = Pose::Unavailable;
        return;
    }
    orientation().last_event_at = now();
    let heading = webkit_compass_heading.or_else(|| alpha.map(|a| (360.0 - a) % 360.0));
    device_orientation_store().set_orientation(heading, beta, gamma);
}

/// True when tracking or asked for. Behind a permission only the shell's
/// prompt can start it.
pub fn request_orientation(platform: OrientationPlatform) -> bool {
    let store = device_orientation_store();
    if store.is_tracking_orientation {
        return true;
    }
    if !platform.supported || store.pose == Pose::Unavailable {
        store.pose = Pose::Unavailable;
        return false;
    }
    if platform.needs_permission {
        return false;
    }
    store.pose = Pose::Loading;
    let o = orientation();
    o.command = Some(OrientationCommand::Listen);
    o.last_event_at = now();
    store.set_is_tracking_orientation(true);
    true
}

/// Without a prompt gate, a grant came from outside the app (device
/// settings) and starts the listener here.
pub fn orientation_permission(answer: PermissionAnswer, platform: OrientationPlatform) {
    let store = device_orientation_store();
    match answer {
        PermissionAnswer::Asking => {
            if !store.is_tracking_orientation {
                store.pose = Pose::Loading;
            }
        }
        PermissionAnswer::Granted if platform.needs_permission => {
            orientation().last_event_at = now();
            store.set_is_tracking_orientation(true);
        }
        PermissionAnswer::Granted => {
            request_orientation(platform);
        }
        PermissionAnswer::Denied => {
            store.set_is_tracking_orientation(false);
            store.pose = Pose::Failed(Message::OrientationPermissionDenied);
        }
        PermissionAnswer::Failed => {
            store.set_is_tracking_orientation(false);
            store.pose = Pose::Failed(Message::OrientationPermissionError);
        }
    }
}

pub fn on_orientation_lost() {
    let store = device_orientation_store();
    store.set_is_tracking_orientation(false);
    store.pose = Pose::Failed(Message::OrientationSignalLost);
}

pub fn orientation_close() {
    orientation().command = Some(OrientationCommand::Remove);
    let store = device_orientation_store();
    store.set_is_tracking_orientation(false);
    store.pose = Pose::Idle;
}

/// Background tabs stop dispatching, so judging the gap on return would read
/// "was hidden" as "lost": take a fresh baseline instead.
pub fn orientation_visible() {
    orientation().last_event_at = now();
}

/// Armed only while a heading is tracked, where silence is the only sign of
/// its loss: nothing tracked, nothing is woken for. It wakes once, where
/// the silence since the last event would be a loss; an event before then
/// moves that deadline, so a talking sensor is never polled. A heading is
/// asked for when the sensors open and again when the user follows
/// (pxl-kernel shell.rs `tracking_action`), never on a timer.
pub fn orientation_watchdog_step(platform: OrientationPlatform) {
    if !platform.silence_is_loss || !device_orientation_store().is_tracking_orientation {
        return;
    }
    let o = orientation();
    let lost_at = o.last_event_at + WATCHDOG_TIMEOUT_MS;
    if now() < lost_at {
        wake_at(lost_at);
        return;
    }
    o.command = Some(OrientationCommand::Remove);
    on_orientation_lost();
}

pub fn take_orientation_command() -> Option<OrientationCommand> {
    orientation().command.take()
}
