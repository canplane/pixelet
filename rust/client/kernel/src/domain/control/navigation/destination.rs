// pxl-kernel/domain/control/navigation/destination.rs

//! Where the camera goes when a place is named by its X and Y (a fix, a
//! search, the start): put there once, on the ground, when the ground is
//! known exactly, never at a guessed height corrected later. A fix's
//! altitude picks the floor nearest it; with none, the surface. A place
//! named with its own height is put as it is (`go_to`).

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::domain::control::camera::controller::{put_center, Motion};
use crate::domain::control::navigation::support::{exact, is_unknown, nearest, unknown_z};
use xpute_guest::global::Global;

static DESTINATION: Global<Option<(Q3, Motion)>> = Global::new();

fn destination() -> &'static mut Option<(Q3, Motion)> {
    DESTINATION.get(|| None)
}

/// A later destination or a `go_to` replaces it. `at`'s Z, where known, is
/// the height whose nearest floor it stands on; unknown, the surface.
pub fn go_to_ground(at: Q3, motion: Motion) {
    *destination() = Some((at, motion));
    crate::sys::sched::edge::request_world_tick();
}

/// A pending destination is dropped: this was asked last.
pub fn go_to(at: Q3, motion: Motion) {
    cancel();
    put_center(at, motion);
}

/// The user put the focus by hand; reaching the destination after would
/// undo it. Turning or zooming leaves it.
pub fn cancel() {
    *destination() = None;
}

/// The place asked for and not yet put.
pub fn pending() -> Option<Q3> {
    destination().map(|(at, _)| at)
}

/// Whose ground the terrain is asked for.
pub fn tile() -> Option<Tile> {
    destination().map(|(at, _)| GROUND.cell_of(LEAF_ZOOM, &at).tile())
}

/// Only the exact ground puts it; a coarser tile over it is not its ground.
pub fn destination_step() {
    let Some((at, motion)) = *destination() else {
        return;
    };
    let Some(to) = (if is_unknown(&at) {
        exact(&at)
    } else {
        nearest(&at)
    }) else {
        return;
    };
    *destination() = None;
    put_center(to, motion);
}

/// A marker's height is its zoom's, coarser than where the center stands, so
/// the center goes on the surface once it is known exactly.
pub(crate) fn go_on_ground(target: Q3, motion: Motion) {
    crate::domain::control::navigation::follow_mode::set_following(false);
    go_to_ground(unknown_z(&target), motion);
}

/// A place, as the location button goes to one: a view far out comes in.
pub fn go_to_place(target: Q3) {
    go_on_ground(target, Motion::Smooth);
    crate::domain::control::camera::controller::come_in_to_default();
}
