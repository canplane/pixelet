// pxl-core/pointer.rs

//! Decides what the pointer is over, here alone and in one order. A marker a
//! tap would act on comes first, because markers are drawn over the world.
//! Then comes whatever the eye's ray meets first in the world. The scene owns
//! the markers and control owns the ray, and neither names the other
//! (domain.rs), so the order is kept here, above both.

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::layer::octant::Octant;

use crate::domain::control::edit::{self, Tool};
use crate::domain::control::navigation::destination;
use crate::domain::layer::object::asset::Asset;
use crate::domain::scene::marker::{self, MarkerHit};

/// What a tap on a scene marker does with the tool held.
enum MarkerTap {
    Go(Q3),
    Remove(Octant),
    Draft(Octant),
}

/// What a tap at `x`, `y` would do to the scene marker there with the tool
/// held. The pointer tool goes to any marker. The eraser removes a pin someone
/// put up, and the marker tool opens that pin's draft. The eraser cannot
/// remove a place's marker.
fn marker_at(x: f64, y: f64) -> Option<MarkerTap> {
    match (edit::tool(), marker::tap(x, y)?) {
        (Tool::Pointer, MarkerHit::To(at) | MarkerHit::Pin(_, at)) => Some(MarkerTap::Go(at)),
        (Tool::Eraser, MarkerHit::Pin(e, _)) => Some(MarkerTap::Remove(e)),
        (
            Tool::Item {
                asset: Asset::Marker,
                ..
            },
            MarkerHit::Pin(e, _),
        ) => Some(MarkerTap::Draft(e)),
        _ => None,
    }
}

pub fn hover(x: f64, y: f64) {
    match marker_at(x, y) {
        Some(_) => edit::point_at(x, y),
        None => edit::hover_at(x, y),
    }
}

/// Whether the pointer is over a marker a tap would act on.
pub fn over_marker() -> bool {
    edit::pointer_ndc().is_some_and(|[x, y]| marker_at(x, y).is_some())
}

pub fn tap(x: f64, y: f64) {
    match marker_at(x, y) {
        Some(MarkerTap::Go(at)) => destination::go_to_place(at),
        Some(MarkerTap::Remove(e)) => edit::write::remove(e),
        Some(MarkerTap::Draft(e)) => edit::write::draft_at(e),
        None => edit::tap(x, y),
    }
}
