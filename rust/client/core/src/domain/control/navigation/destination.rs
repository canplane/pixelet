// pxl-core/domain/control/navigation/destination.rs

//! Where the camera goes when a place is named by its X and Y (a location
//! fix, a search, the start). The pivot is put there once, on the ground,
//! when the ground there is known exactly. A fix's altitude picks the floor
//! nearest it, and a place without one goes on the surface. A place named
//! with its own height is put as it is (`go_to`).

use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM};

use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::{put_pivot, Motion};
use crate::domain::control::camera::state::IntentPatch;
use crate::domain::control::navigation::follow_mode;
use crate::domain::control::support::{exact, is_unknown, nearest, unknown_z};
use crate::sys::sched::edge;
use xpute_core::global::Global;

static DST: Global<Option<(Q3, Motion)>> = Global::new();

fn dst() -> &'static mut Option<(Q3, Motion)> {
    DST.get(|| None)
}

/// Sends the pivot to the ground at `at`, once the ground there is known. A
/// later destination or a `go_to` replaces this one. Where the Z of `at` is
/// known, the pivot stands on the floor nearest that height. Where it is
/// unknown, the pivot stands on the surface.
pub fn go_to_ground(at: Q3, motion: Motion) {
    *dst() = Some((at, motion));
    edge::request_world_tick();
}

/// Puts the pivot at `at` as it is. A pending destination is dropped,
/// because this was asked last.
pub fn go_to(at: Q3, motion: Motion) {
    cancel();
    put_pivot(at, motion);
}

/// Drops the pending destination when the user puts the pivot by hand,
/// since reaching it afterwards would undo the user's move. Turning or
/// zooming keeps it.
pub fn cancel() {
    *dst() = None;
}

/// The place asked for and not yet put.
pub fn pending() -> Option<Q3> {
    dst().map(|(at, _)| at)
}

/// The leaf tile under the pending destination. The terrain is asked for
/// it, so the destination's ground arrives.
pub fn tile() -> Option<Quadrant> {
    dst().map(|(at, _)| GROUND.cell_of(LEAF_ZOOM, &at).tile())
}

/// Puts the pivot at the pending destination once the ground there is known
/// exactly. A coarser tile's answer is not the destination's ground, so it
/// puts nothing.
pub fn dst_step() {
    let Some((at, motion)) = *dst() else {
        return;
    };
    let Some(to) = (if is_unknown(&at) {
        exact(&at)
    } else {
        nearest(&at)
    }) else {
        return;
    };
    *dst() = None;
    put_pivot(to, motion);
}

/// Sends the pivot to the surface at the X and Y of `to`, and stops
/// following the device. A marker's height is its zoom's, coarser than where
/// the pivot stands, so the pivot goes on the surface once that is known
/// exactly.
pub(crate) fn go_on_ground(to: Q3, motion: Motion) {
    follow_mode::set_following(false);
    go_to_ground(unknown_z(&to), motion);
}

/// Goes to a place as the location button does. A view far out comes in to
/// the default distance.
pub fn go_to_place(to: Q3) {
    go_on_ground(to, Motion::Smooth);
    controller::come_in_to_default();
}

/// Goes to a search's place at `lon`, `lat` degrees, with the view asked
/// with it. A view far out comes in to the default distance, and then the
/// view is asked, so a distance asked with the search wins over the coming
/// in.
pub fn go_to_deg(lon: f64, lat: f64, view: IntentPatch) {
    go_on_ground(epsg4326::project(&[lon, lat, 0.0]), Motion::Snap);
    controller::come_in_to_default();
    controller::set_intent(view);
}
