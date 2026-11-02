// pxl-kernel/pointer.rs

//! What the pointer is over, decided here alone and in one order: a marker a
//! tap would act on, drawn over the world, then what the eye's ray meets
//! first in the world. What is met first takes the pointer. The scene and
//! control name neither the other (domain.rs), so the order is kept above
//! both.

use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::q3::Q3;

use crate::domain::control::edit::{self, ActiveTool};
use crate::domain::control::navigation::destination;
use crate::domain::scene::marker::{self, MarkerHit};

/// What a tap on a scene marker does with the tool held.
enum MarkerTap {
    Go(Q3),
    Remove(EntryAt),
    Draft(EntryAt),
}

/// The scene marker at `x`, `y` a tap would act on with the tool held: the
/// pointer goes to any, the eraser takes away a pin someone put up, the marker
/// tool opens that pin's draft. A place's scene marker is no eraser's.
fn marker_at(x: f64, y: f64) -> Option<MarkerTap> {
    match (edit::tool().active_tool, marker::tap(x, y)?) {
        (ActiveTool::Pointer, MarkerHit::To(at) | MarkerHit::Pin(_, at)) => Some(MarkerTap::Go(at)),
        (ActiveTool::Eraser, MarkerHit::Pin(e, _)) => Some(MarkerTap::Remove(e)),
        (ActiveTool::Marker, MarkerHit::Pin(e, _)) => Some(MarkerTap::Draft(e)),
        _ => None,
    }
}

/// The pointer stands at `x`, `y`.
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

/// A tap at `x`, `y`.
pub fn tap(x: f64, y: f64) {
    match marker_at(x, y) {
        Some(MarkerTap::Go(at)) => destination::go_to_place(at),
        Some(MarkerTap::Remove(e)) => edit::write::remove(e),
        Some(MarkerTap::Draft(e)) => edit::write::draft_at(e),
        None => edit::tap(x, y),
    }
}
