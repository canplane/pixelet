// pxl_core/module/device/pose/use_device_orientation.rs

//! The device's compass sensor, as decisions: the heading an event means, when
//! to start listening, and when a silent sensor counts as lost. The listener is
//! the shell's — it takes `take_orientation_commands`, asks iOS's permission
//! inside a gesture, and reports events back.

use pxl_shared::spec::message::Message;

use super::store::{use_device_orientation_store, DevicePoseStatus};
use crate::sys::global::{now, Global};

/// There's no Permissions-API equivalent for orientation (iOS Safari doesn't
/// expose one at all — permission there is only ever regrantable via a fresh
/// user gesture, by design), so a permission revoked mid-session can't be pushed
/// to us like geolocation's `change` event. Track the last real event instead
/// and treat a long gap while nominally tracking as the signal having been lost.
const WATCHDOG_TIMEOUT_MS: f64 = 8000.0;
const WATCHDOG_INTERVAL_MS: f64 = 3000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrientationCommand {
    /// Listen: `absolute` for deviceorientationabsolute where the browser has it.
    Listen,
    /// Ask iOS's permission, inside the current user gesture, then listen.
    RequestPermission,
    Remove,
}

/// What the platform offers, as the shell found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrientationPlatform {
    pub supported: bool,
    /// iOS 13+'s permission gate: requestPermission exists and must run inside
    /// a direct user gesture.
    pub needs_permission: bool,
}

struct Orientation {
    subscribers: u32,
    last_event_at: Option<f64>,
    watchdog_at: f64,
    commands: Vec<OrientationCommand>,
}

static ORIENTATION: Global<Orientation> = Global::new();

fn orientation() -> &'static mut Orientation {
    ORIENTATION.get(|| Orientation {
        subscribers: 0,
        last_event_at: None,
        watchdog_at: 0.0,
        commands: Vec::new(),
    })
}

/// An orientation event: iOS's own compass heading preferred, the alpha-derived
/// one otherwise.
pub fn orientation_event(webkit_compass_heading: Option<f64>, alpha: Option<f64>, beta: Option<f64>, gamma: Option<f64>) {
    orientation().last_event_at = Some(now());
    let heading = webkit_compass_heading.or_else(|| alpha.map(|a| (360.0 - a) % 360.0));
    use_device_orientation_store().set_orientation(heading, beta, gamma);
}

/// Requests the sensor, guarded against duplicate activation: true when it is
/// tracking or has been asked for.
pub fn request_orientation(platform: OrientationPlatform) -> bool {
    let store = use_device_orientation_store();
    if store.is_tracking_orientation {
        return true;
    }
    if !platform.supported {
        store.set_error(Message::OrientationUnsupported);
        return false;
    }
    store.set_status(DevicePoseStatus::Loading);
    let o = orientation();
    if platform.needs_permission {
        o.commands.push(OrientationCommand::RequestPermission);
        return false;
    }
    o.commands.push(OrientationCommand::Listen);
    o.last_event_at = Some(now());
    store.set_is_tracking_orientation(true);
    true
}

/// iOS's answer to the permission request.
pub fn orientation_permission(granted: bool, failed: bool) {
    let store = use_device_orientation_store();
    if granted {
        orientation().last_event_at = Some(now());
        store.set_is_tracking_orientation(true);
    } else if failed {
        store.set_error(Message::OrientationPermissionError);
    } else {
        store.set_error(Message::OrientationPermissionDenied);
    }
}

/// A subscriber arrived; auto-started on platforms that allow it.
pub fn orientation_subscribe(platform: OrientationPlatform) {
    orientation().subscribers += 1;
    if !use_device_orientation_store().is_tracking_orientation && !platform.needs_permission {
        request_orientation(platform);
    }
}

/// A subscriber left; the last removes the listener.
pub fn orientation_unsubscribe() {
    let o = orientation();
    o.subscribers = o.subscribers.saturating_sub(1);
    if o.subscribers == 0 {
        o.commands.push(OrientationCommand::Remove);
        let store = use_device_orientation_store();
        if store.is_tracking_orientation {
            store.set_is_tracking_orientation(false);
        }
        if store.status != DevicePoseStatus::Idle {
            store.set_status(DevicePoseStatus::Idle);
        }
    }
}

/// Browsers throttle or stop dispatching deviceorientation for backgrounded
/// tabs, so the last event goes stale the whole time we're hidden; judging the
/// gap right as the tab becomes visible would misread "was backgrounded" as
/// "signal lost". A fresh baseline instead.
pub fn orientation_visible() {
    let o = orientation();
    if o.last_event_at.is_some() {
        o.last_event_at = Some(now());
    }
}

/// The watchdog's turn, while the tab is visible: polls for a stalled signal
/// while nominally tracking, and doubles as this sensor's only path back once
/// marked lost — request_orientation no-ops while already tracking, so retrying
/// is safe to repeat. iOS's request must run inside a user gesture, so recovery
/// there stays manual (the compass button).
pub fn orientation_watchdog_step(platform: OrientationPlatform) {
    let o = orientation();
    let t = now();
    if t < o.watchdog_at {
        return;
    }
    o.watchdog_at = t + WATCHDOG_INTERVAL_MS;
    let store = use_device_orientation_store();
    if !store.is_tracking_orientation {
        if !platform.needs_permission {
            request_orientation(platform);
        }
        return;
    }
    let Some(last) = o.last_event_at else { return };
    if t - last > WATCHDOG_TIMEOUT_MS {
        o.commands.push(OrientationCommand::Remove);
        store.set_is_tracking_orientation(false);
        store.set_error(Message::OrientationSignalLost);
        o.last_event_at = None;
    }
}

/// What the shell's listener should do, in order.
pub fn take_orientation_commands() -> Vec<OrientationCommand> {
    core::mem::take(&mut orientation().commands)
}
