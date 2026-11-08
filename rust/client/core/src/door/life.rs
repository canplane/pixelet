// pxl-core/door/life.rs

//! Applies the life events the runtime sends as xpute's SYS commands: a
//! runtime attached, the core about to be frozen, and the core woken.

use super::{on_runtime_attached, State};
use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::geolocation;
use crate::domain::layer::object::repository;
use crate::domain::view::setting::store::settings_open;
use crate::sys::sched::edge;
use xpute_core::ipc::door::{Life, Outcome};

pub(super) fn apply(s: &mut State, life: Life) -> Outcome {
    match life {
        Life::Attach => {
            on_runtime_attached(s);
            // The settings' slot is opened here and read by the turns that
            // follow. What waits on the settings, the profiler and the
            // snapshot, starts once they are in (turn.rs). The objects are
            // not read here: they are read a shard at a time as the view asks
            // for them (domain/layer/object/repository.rs).
            settings_open();
            // The UI must hold the core's state from its first render on.
            edge::request_world_tick();
            s.snapshot_due = true;
        }
        // Suspend is the last moment a save is sure to happen. The objects
        // are written, and the position is no longer watched while nothing
        // shows it.
        Life::Suspend => {
            geolocation::geolocation_visibility(false);
            repository::save();
        }
        Life::Resume => {
            geolocation::geolocation_visibility(true);
            device_orientation::orient_visible();
        }
    }
    Ok(Some(0))
}
