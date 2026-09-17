// pxl_core/module/map/ui/toast/toast_controller.rs

//! Which status toasts show and what they say: the device's location, and the
//! world's readiness — the map's tiles and the companion's first load merged
//! into one, since both mean "is the world I'm looking at ready yet". The shell
//! renders a toast by its id and message key; a message key rather than a
//! message, because the catalogue loads over the network and a resolved string
//! would freeze before a later language change.

use pxl_shared::spec::message::Message;

use crate::module::device::pose::store::{device_location_store, DevicePoseStatus};
use crate::module::map::store::{map_store, MapStatus};
use crate::module::world::change::wake_at;
use crate::module::world::character::store::companion_load_store;
use crate::sys::global::{now, Global};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastTone {
    Loading,
    Error,
    Success,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastSpec {
    pub id: &'static str,
    pub message: Message,
    pub tone: ToastTone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToastCommand {
    Show(ToastSpec),
    Dismiss(&'static str),
}

const WORLD_READY_MS: f64 = 3000.0;

struct Controller {
    shown_location: bool,
    location_for: Option<(DevicePoseStatus, Option<Message>)>,
    world_for: Option<(MapStatus, bool)>,
    /// Whether the combined loading/error state was active last run — the
    /// transition out of it is what distinguishes "just finished, flash
    /// success" from "never started, nothing to show".
    was_world_loading: bool,
    /// Once the companion has completed its very first load, every later dip
    /// is a variant swap in settings — a costume change, not the world going
    /// unready.
    companion_first_load_done: bool,
    dismiss_world_at: Option<f64>,
    commands: Vec<ToastCommand>,
}

static CONTROLLER: Global<Controller> = Global::new();

/// The controller's turn.
pub fn toast_controller_step() {
    let c = CONTROLLER.get(|| Controller {
        shown_location: false,
        location_for: None,
        world_for: None,
        was_world_loading: false,
        companion_first_load_done: false,
        dismiss_world_at: None,
        commands: Vec::new(),
    });

    let location = device_location_store();
    let location_now = (location.status, location.error);
    if c.location_for != Some(location_now) {
        c.location_for = Some(location_now);
        let is_loading = location.status == DevicePoseStatus::Loading;
        let is_error = location.status == DevicePoseStatus::Error;
        if !is_loading && !is_error {
            if c.shown_location {
                c.commands.push(ToastCommand::Dismiss("location-status"));
                c.shown_location = false;
            }
        } else {
            c.shown_location = true;
            c.commands.push(ToastCommand::Show(ToastSpec {
                id: "location-status",
                message: if is_error {
                    location.error.unwrap_or(Message::LocationWeakSignal)
                } else {
                    Message::LocationChecking
                },
                tone: if is_error { ToastTone::Error } else { ToastTone::Loading },
            }));
        }
    }

    let map_status = map_store().map_status;
    let companion = companion_load_store();
    let companion_loading = !companion.person_ready || !companion.pet_ready;
    if c.world_for != Some((map_status, companion_loading)) {
        c.world_for = Some((map_status, companion_loading));
        let is_map_loading = map_status == MapStatus::Loading;
        let is_error = map_status == MapStatus::Error;
        let relevant_companion_loading = companion_loading && !c.companion_first_load_done;
        if !companion_loading {
            c.companion_first_load_done = true;
        }
        let is_loading = is_map_loading || relevant_companion_loading;
        if is_loading || is_error {
            c.was_world_loading = true;
            c.dismiss_world_at = None;
            // Map load takes priority in the message when both are in flight —
            // it's the more foundational concern.
            let message = if is_error {
                Message::MapLoadError
            } else if is_map_loading {
                Message::MapLoading
            } else {
                Message::CompanionLoading
            };
            c.commands.push(ToastCommand::Show(ToastSpec {
                id: "world-status",
                message,
                tone: if is_error { ToastTone::Error } else { ToastTone::Loading },
            }));
        } else if c.was_world_loading {
            c.was_world_loading = false;
            c.commands.push(ToastCommand::Show(ToastSpec {
                id: "world-status",
                message: Message::WorldReady,
                tone: ToastTone::Success,
            }));
            c.dismiss_world_at = Some(now() + WORLD_READY_MS);
        } else {
            c.dismiss_world_at = None;
        }
    }

    match c.dismiss_world_at {
        Some(at) if now() >= at => {
            c.dismiss_world_at = None;
            c.commands.push(ToastCommand::Dismiss("world-status"));
        }
        Some(at) => wake_at(at),
        None => {}
    }
}

/// What the shell should show or dismiss, in order.
pub fn take_toast_commands() -> Vec<ToastCommand> {
    CONTROLLER
        .get(|| Controller {
            shown_location: false,
            location_for: None,
            world_for: None,
            was_world_loading: false,
            companion_first_load_done: false,
            dismiss_world_at: None,
            commands: Vec::new(),
        })
        .commands
        .drain(..)
        .collect()
}
