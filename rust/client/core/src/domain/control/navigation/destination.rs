// pxl-core/domain/control/navigation/destination.rs

//! Where the camera goes when a place is named by its X and Y (a location
//! fix, a search, the start). The pivot is put there once, on the ground,
//! when the ground there is known exactly. A fix's altitude picks the floor
//! nearest it, and a place without one goes on the surface. A place named
//! with its own height is put as it is (`go_to`).

use crate::spec::step::InvalidationReason;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::{GROUND, Grid};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::control::camera::controller::{Motion, put_pivot};
use crate::domain::control::support::arriving;
use crate::sys::sched;
use pxl_global::Global;

static DST: Global<Option<(Q3, Motion)>> = Global::new();

fn dst() -> &'static mut Option<(Q3, Motion)> {
    DST.get(|| None)
}

/// Sends the pivot to the ground at `at`, once the ground there is known. A
/// later destination or a `go_to` replaces this one. Where the Z of `at` is
/// known, the pivot stands on the floor nearest that height. Where it is
/// unknown, the pivot stands on the surface.
pub(crate) fn go_to_ground(at: Q3, motion: Motion) {
    *dst() = Some((at, motion));
    sched::invalidate(InvalidationReason::View);
}

/// Puts the pivot at `at` as it is. A pending destination is dropped,
/// because this was asked last.
pub(crate) fn go_to(at: Q3, motion: Motion) {
    cancel();
    put_pivot(at, motion);
}

/// Drops the pending destination when the user puts the pivot by hand,
/// since reaching it afterwards would undo the user's move. Turning or
/// zooming keeps it.
pub(crate) fn cancel() {
    *dst() = None;
}

pub(crate) fn pending() -> Option<Q3> {
    dst().map(|(at, _)| at)
}

/// The leaf tile under the pending destination. The terrain is asked for
/// it, so the destination's ground arrives.
pub(crate) fn tile() -> Option<Quadrant> {
    dst().map(|(at, _)| GROUND.cell_of(LEAF_ZOOM, &at).tile())
}

/// Puts the pivot at the pending destination once the ground there is known
/// exactly. A coarser tile's answer is not the destination's ground, so it
/// puts nothing.
pub(crate) fn dst_step() {
    let Some((at, motion)) = *dst() else {
        return;
    };
    let Some(to) = arriving(&at) else {
        return;
    };
    *dst() = None;
    put_pivot(to, motion);
}
