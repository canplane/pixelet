// pxl-core/domain/control/sensor/device_orientation.rs

//! The compass sensor's decisions: the heading an event means, when to
//! start it, and when silence counts as a lost sensor. The UI owns the
//! listener and iOS's permission prompt, and the core hears only the answer.

use crate::abi::overlay::SensorStatus;
use crate::abi::sensor::PermissionAnswer;
use crate::domain::control::param::WATCHDOG_TIMEOUT_MS;
use crate::domain::control::sensor::reading::{device_orient_store, device_orient_store_mut};
use crate::sys::sched::invalidate_at;
use pxl_global::Global;
use xpute_guest::clock::now;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OrientationCommand {
    /// The UI starts the compass, deviceorientationabsolute where the browser
    /// has it.
    Start,
    Stop,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct OrientationPlatform {
    pub supported: bool,
    /// On iOS 13 and later, `requestPermission` must run inside a user
    /// gesture.
    pub needs_permission: bool,
    /// Whether silence means the sensor is lost. A browser's sensor talks
    /// even while the device is still. A device's sensor (Core Location)
    /// speaks only on change and reports its own failure.
    pub silence_is_loss: bool,
}

static PLATFORM: Global<OrientationPlatform> = Global::new();

/// The compass as the UI reports it once the core is attached. Until then
/// the place has none.
pub(crate) fn platform() -> &'static mut OrientationPlatform {
    PLATFORM.get(OrientationPlatform::default)
}

struct Orientation {
    /// The time the watchdog measures silence from: the last event, the
    /// start of tracking, or the page's return to view. It is read only while
    /// tracking, and every start of tracking sets it.
    last_event_at: f64,
    /// The frame's last command. The UI's listener follows it and ignores a
    /// repeated command.
    cmd: Option<OrientationCommand>,
}

static ORIENT: Global<Orientation> = Global::new();

fn orient() -> &'static mut Orientation {
    ORIENT.get(|| Orientation {
        last_event_at: 0.0,
        cmd: None,
    })
}

/// An orientation event, and whether it changed the heading held. iOS's
/// compass heading is preferred, and otherwise the heading is derived from
/// alpha.
pub(crate) fn orient_event(webkit_compass_heading: Option<f64>, alpha: Option<f64>) -> bool {
    // An event with neither is a reading with no heading. One with no field at
    // all is a page's word that it has no compass, which its reader tells
    // apart (`orient_unavailable`).
    let heading = webkit_compass_heading.or_else(|| alpha.map(|a| (360.0 - a) % 360.0));
    orient().last_event_at = now();
    device_orient_store_mut().set_orient(heading)
}

/// The place has no compass, as a page's event with every field null says: its
/// listener stops, and the compass reads unavailable, which `request_orient`
/// keeps until the world's surface closes.
pub(crate) fn orient_unavailable() {
    orient().cmd = Some(OrientationCommand::Stop);
    let store = device_orient_store_mut();
    store.set_is_tracking_orient(false);
    store.status = SensorStatus::Unavailable;
}

/// Asks for the heading. Returns true when the heading is tracked or now
/// asked for. Behind a permission, only the UI's prompt can start it, so
/// this returns false there.
pub(crate) fn request_orient() -> bool {
    let store = device_orient_store_mut();
    if store.is_tracking_orient {
        return true;
    }
    if !platform().supported || store.status == SensorStatus::Unavailable {
        store.status = SensorStatus::Unavailable;
        return false;
    }
    if platform().needs_permission {
        return false;
    }
    store.status = SensorStatus::Loading;
    let o = orient();
    o.cmd = Some(OrientationCommand::Start);
    o.last_event_at = now();
    store.set_is_tracking_orient(true);
    true
}

/// The answer to the orientation permission. Where no prompt gates the
/// sensor, a grant came from outside the app (the device's settings) and
/// starts the listener here.
pub(crate) fn orient_permission(answer: PermissionAnswer) {
    let store = device_orient_store_mut();
    match answer {
        PermissionAnswer::Asking => {
            if !store.is_tracking_orient {
                store.status = SensorStatus::Loading;
            }
        }
        PermissionAnswer::Granted if platform().needs_permission => {
            orient().last_event_at = now();
            store.set_is_tracking_orient(true);
        }
        PermissionAnswer::Granted => {
            request_orient();
        }
        PermissionAnswer::Denied | PermissionAnswer::Failed => {
            store.set_is_tracking_orient(false);
            store.status = SensorStatus::Error;
        }
    }
}

pub(crate) fn on_orient_lost() {
    let store = device_orient_store_mut();
    store.set_is_tracking_orient(false);
    store.status = SensorStatus::Error;
}

pub(crate) fn orient_close() {
    orient().cmd = Some(OrientationCommand::Stop);
    let store = device_orient_store_mut();
    store.set_is_tracking_orient(false);
    store.status = SensorStatus::Idle;
}

/// Called when the page comes back into view. Background tabs stop
/// dispatching events, so judging the gap on return would mistake a hidden
/// page for a lost sensor. The watchdog measures afresh from now instead.
pub(crate) fn orient_visible() {
    orient().last_event_at = now();
}

/// Judges whether the heading is lost. It is armed only while a heading is
/// tracked on a platform where silence is the only sign of loss, so nothing
/// is woken while nothing is tracked. It wakes once, when the silence since
/// the last event would amount to a loss. An event before then moves that
/// deadline, so a sensor that keeps talking is never polled.
pub(crate) fn orient_watchdog_step() {
    if !platform().silence_is_loss || !device_orient_store().is_tracking_orient {
        return;
    }
    let o = orient();
    let lost_at = o.last_event_at + WATCHDOG_TIMEOUT_MS;
    if now() < lost_at {
        invalidate_at(lost_at);
        return;
    }
    o.cmd = Some(OrientationCommand::Stop);
    on_orient_lost();
}

pub(crate) fn take_orient_cmd() -> Option<OrientationCommand> {
    orient().cmd.take()
}
