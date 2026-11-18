// pxl-core/domain/control/navigation/destination.rs

//! Where the camera goes when a place is named by its X and Y (a location
//! fix, a search, the start). The pivot is put there once a tile held gives
//! the ground there exactly, on the surface: the camera stands it as the eye
//! reads the ground there (control/support.rs). A fix's altitude picks the
//! floor nearest it, which the eye reads once the camera is there. A place
//! named with its own height is put as it is (`go_to`).

use crate::spec::step::InvalidationReason;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::{GROUND, Grid};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::control::camera::controller::{Motion, pivot_to, put_pivot};
use crate::domain::layer::Base as _;
use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use crate::domain::query::{World, is_unknown, on_floor, unknown};
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

/// Puts the pivot at the pending destination once a tile held gives the
/// ground there exactly (`TileCache::src`); a coarser tile over it is not the
/// destination's ground, so it puts nothing. A place with no height is put on
/// the surface and is reached. One with a height is reached on the floor
/// nearest that height, which the eye reads exactly only from near it: until
/// it does, the pivot is put over the place on the surface, and the camera
/// brings the eye there.
pub(crate) fn dst_step() {
    let Some((at, motion)) = *dst() else {
        return;
    };
    let leaf = GROUND.cell_of(LEAF_ZOOM, &at).tile();
    if IndexedTerrainTile::tile_cache().src(leaf).is_none() {
        return;
    }
    let over = unknown(&at);
    let reached = match is_unknown(&at) {
        true => Some(at),
        false => {
            let nearest = World::now().support_nearest(&at).exact().flatten();
            nearest.map(|top| on_floor(&at, top))
        }
    };
    match reached {
        Some(to) => {
            *dst() = None;
            put_pivot(to, motion);
        }
        None if pivot_to().is_none_or(|p| unknown(&p) != over) => put_pivot(over, motion),
        None => {}
    }
}

#[cfg(test)]
#[path = "destination.test.rs"]
mod test;
