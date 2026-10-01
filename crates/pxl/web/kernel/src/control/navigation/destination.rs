// pxl-kernel/control/navigation/destination.rs

//! Where the camera is going when only a place on the map is named, its X
//! and Y — the device's fix, a search result, where the map starts — and not
//! its height. The camera is put there once, standing on the ground there as
//! the ground is known exactly, and not before: never at a height guessed
//! and corrected when the ground arrives, since the ground arriving moves no
//! camera. Until then the terrain asks for the pages that say it
//! (world/pyramid.rs `Pyramid::step`), with or without a view, and the camera
//! stays where it is, or unplaced at the first fix.
//!
//! A place named with its height — a badge's, an object's — is no
//! destination: it is put as it is (`go_to`).

use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{LevelPage, GROUND};

use crate::control::camera::controller::{put_center, Motion};
use crate::global::Global;
use crate::layer::terrain::ground_snap::pivot_over;
use crate::world::access::{Access, FINEST};

/// The place named and not yet reached, its Z meaningless, and how it is to
/// be reached.
static DESTINATION: Global<Option<(Pvf3, Motion)>> = Global::new();

fn destination() -> &'static mut Option<(Pvf3, Motion)> {
    DESTINATION.get(|| None)
}

/// The camera to stand over `at`'s X and Y, on the ground there, once that
/// is known; its Z says nothing. A later destination or a `go_to` takes its
/// place.
pub fn go_to_ground(at: Pvf3, motion: Motion) {
    *destination() = Some((at, motion));
    crate::world::change::request_world_tick();
}

/// The camera put at `at` as it is: a place named with its height. A
/// destination still waiting is dropped, since this is where the camera was
/// asked to be last.
pub fn go_to(at: Pvf3, motion: Motion) {
    cancel();
    put_center(at, motion);
}

/// The destination dropped: the user has put the map center somewhere
/// themselves, and a destination reached after that would undo it. What
/// only turns or zooms about the center leaves it standing.
pub fn cancel() {
    *destination() = None;
}

/// The z16 page the destination stands in, whose ground the terrain is
/// asked for.
pub fn page() -> Option<LevelPage> {
    destination().map(|(at, _)| GROUND.cell_of(FINEST, &at).page())
}

/// The destination reached where its ground is known exactly: the camera put
/// there, once. A coarser page over it is not its ground and puts nothing.
pub fn destination_step() {
    let Some((at, motion)) = *destination() else { return };
    let Some(top) = Access::now().support(&at).exact() else { return };
    *destination() = None;
    put_center(pivot_over(&at, top), motion);
}
