// pxl-kernel/pointer.rs

//! What the pointer is over, decided here alone and in one order: a marker a
//! tap would act on, drawn over the world, then what the eye's ray meets
//! first in the world. What is met first takes the pointer, and nothing
//! behind it is picked, previewed or tapped. The scene and control name
//! neither the other (domain.rs), so the order is kept above both.

use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::q3::Q3;

use crate::domain::control::interaction::{self, ActiveTool};
use crate::domain::scene::marker::{self, Went};

/// What a tap on a marker does with the tool held.
enum MarkerTap {
    Go(Q3),
    Remove(EntryAt),
    Wait(EntryAt),
}

/// The marker at `x`, `y` a tap would act on with the tool held: the pointer
/// goes to any, the eraser takes away a pin someone put up, the marker tool
/// has the waiting pin stand there. A place's marker is no eraser's.
fn marker_at(x: f64, y: f64) -> Option<MarkerTap> {
    match (interaction::holding().active_tool, marker::tap(x, y)?) {
        (ActiveTool::Pointer, Went::To(at) | Went::Pin(_, at)) => Some(MarkerTap::Go(at)),
        (ActiveTool::Eraser, Went::Pin(e, _)) => Some(MarkerTap::Remove(e)),
        (ActiveTool::Marker, Went::Pin(e, _)) => Some(MarkerTap::Wait(e)),
        _ => None,
    }
}

/// The pointer stands at `x`, `y`.
pub fn hover(x: f64, y: f64) {
    match marker_at(x, y) {
        Some(_) => interaction::point_at(x, y),
        None => interaction::hover_at(x, y),
    }
}

/// Whether the pointer is over a marker a tap would act on.
pub fn over_marker() -> bool {
    interaction::pointed().is_some_and(|[x, y]| marker_at(x, y).is_some())
}

/// A tap at `x`, `y`.
pub fn tap(x: f64, y: f64) {
    match marker_at(x, y) {
        Some(MarkerTap::Go(at)) => crate::domain::control::navigation::destination::go_to_place(at),
        Some(MarkerTap::Remove(e)) => interaction::edit::remove(e),
        Some(MarkerTap::Wait(e)) => interaction::edit::wait_at(e),
        None => interaction::tap(x, y),
    }
}
