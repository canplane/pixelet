// pxl-kernel/door/life.rs

//! The kernel's life as its runtime tells it (xpute's SYS commands): a runtime attached,
//! the kernel about to be frozen, and woken.

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
            // The settings' slot, read from a turn on: what waits on the
            // settings, the profiler and the snapshot's, starts once they
            // are in (turn.rs). The objects are read a shard at a time as
            // the view asks for them (domain/layer/object/repository.rs).
            settings_open();
            // The shell holds the kernel's state from its first render on.
            edge::request_world_tick();
            s.snapshot_due = true;
        }
        // The last sure moment to save: the objects written, and the
        // position no longer watched while nothing shows it.
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
