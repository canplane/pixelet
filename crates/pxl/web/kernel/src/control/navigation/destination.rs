// pxl-kernel/control/navigation/destination.rs

//! Where the camera goes when only a place's X and Y are named (a fix, a
//! search, the start): put there once, on the ground, when the ground is
//! known exactly, never at a guessed height corrected later. A place named
//! with its height is put as it is (`go_to`).

use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{Page, GROUND, LEAF_Z};

use crate::control::camera::controller::{put_center, Motion};
use crate::control::navigation::support::{exact, unknown_z};
use xpute_runtime::global::Global;

static DESTINATION: Global<Option<(Pvf3, Motion)>> = Global::new();

fn destination() -> &'static mut Option<(Pvf3, Motion)> {
    DESTINATION.get(|| None)
}

/// A later destination or a `go_to` replaces it.
pub fn go_to_ground(at: Pvf3, motion: Motion) {
    *destination() = Some((unknown_z(&at), motion));
    crate::world::change::request_world_tick();
}

/// A pending destination is dropped: this was asked last.
pub fn go_to(at: Pvf3, motion: Motion) {
    cancel();
    put_center(at, motion);
}

/// The user put the map center by hand; reaching the destination after would
/// undo it. Turning or zooming leaves it.
pub fn cancel() {
    *destination() = None;
}

/// Whose ground the terrain is asked for.
pub fn page() -> Option<Page> {
    destination().map(|(at, _)| GROUND.cell_of(LEAF_Z, &at).page())
}

/// Only the exact ground puts it; a coarser page over it is not its ground.
pub fn destination_step() {
    let Some((at, motion)) = *destination() else { return };
    let Some(to) = exact(&at) else { return };
    *destination() = None;
    put_center(to, motion);
}
