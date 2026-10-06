// pxl-kernel/control/sensor/device_orientation.rs

//! The compass sensor's decisions: the heading an event means, when to
//! listen, and when silence counts as lost. The listener and iOS's permission
//! prompt are the shell's; the kernel hears only the answer.

use crate::abi::cmd::Message;

use crate::control::sensor::store::{device_orientation_store, PermissionAnswer, Pose};
use crate::world::change::wake_at;
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

/// Orientation has no Permissions API, so revocation cannot be pushed; a long
/// gap while tracking is taken as loss. Only valid where the sensor talks
/// while still (`silence_is_loss`).
const WATCHDOG_TIMEOUT_MS: f64 = 8000.0;
const WATCHDOG_INTERVAL_MS: f64 = 3000.0;

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
    last_event_at: Option<f64>,
    watchdog_at: f64,
    commands: Vec<OrientationCommand>,
}

static ORIENTATION: Global<Orientation> = Global::new();

fn orientation() -> &'static mut Orientation {
    ORIENTATION.get(|| Orientation {
        last_event_at: None,
        watchdog_at: 0.0,
        commands: Vec::new(),
    })
}

/// iOS's compass heading preferred, the alpha-derived one otherwise.
pub fn orientation_event(webkit_compass_heading: Option<f64>, alpha: Option<f64>, beta: Option<f64>, gamma: Option<f64>) {
    // An API with no sensor answers once, empty, and never again.
    if webkit_compass_heading.is_none() && alpha.is_none() && beta.is_none() && gamma.is_none() {
        let o = orientation();
        o.last_event_at = None;
        o.commands.push(OrientationCommand::Remove);
        let store = device_orientation_store();
        store.set_is_tracking_orientation(false);
        store.pose = Pose::Unavailable;
        return;
    }
    orientation().last_event_at = Some(now());
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
    o.commands.push(OrientationCommand::Listen);
    o.last_event_at = Some(now());
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
            orientation().last_event_at = Some(now());
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

pub fn orientation_lost() {
    orientation().last_event_at = None;
    let store = device_orientation_store();
    store.set_is_tracking_orientation(false);
    store.pose = Pose::Failed(Message::OrientationSignalLost);
}

pub fn orientation_close() {
    orientation().commands.push(OrientationCommand::Remove);
    let store = device_orientation_store();
    store.set_is_tracking_orientation(false);
    store.pose = Pose::Idle;
}

/// Background tabs stop dispatching, so judging the gap on return would read
/// "was hidden" as "lost": take a fresh baseline instead.
pub fn orientation_visible() {
    let o = orientation();
    if o.last_event_at.is_some() {
        o.last_event_at = Some(now());
    }
}

/// Also the only path back once lost; iOS recovery stays manual (the compass
/// button), since its request needs a user gesture.
pub fn orientation_watchdog_step(platform: OrientationPlatform) {
    if !platform.silence_is_loss {
        return;
    }
    let o = orientation();
    let t = now();
    if t < o.watchdog_at {
        wake_at(o.watchdog_at);
        return;
    }
    o.watchdog_at = t + WATCHDOG_INTERVAL_MS;
    wake_at(o.watchdog_at);
    let store = device_orientation_store();
    if !store.is_tracking_orientation {
        if !platform.needs_permission {
            request_orientation(platform);
        }
        return;
    }
    let Some(last) = o.last_event_at else { return };
    if t - last > WATCHDOG_TIMEOUT_MS {
        o.commands.push(OrientationCommand::Remove);
        orientation_lost();
    }
}

pub fn take_orientation_commands() -> Vec<OrientationCommand> {
    core::mem::take(&mut orientation().commands)
}
