// pxl-core/domain/control/navigation/place.rs

//! A place made a point. A place is named by its X and Y: on the surface
//! there, or, named with a height such as a fix's altitude, on the floor
//! nearest that height; a tap names a cell, and its point is on the floor
//! under it. The point is the ground's own, asked at the leaves
//! (`SizeBy::Leaf`), so a place is a point only once the world holds the
//! leaves over it, and that point does not change as the camera goes there.

use pxl_global::Global;
use pxl_spec::space::coord::q3::Q3;

use crate::domain::layer::Answer;
use crate::domain::query::change::Memo;
use crate::domain::query::{SizeBy, World, is_unknown, on_floor};

/// Which floor a place is made a point on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Floor {
    /// The surface, or, where the place has a height, such as a fix's
    /// altitude, the floor nearest it.
    Ground,
    /// The floor under the place's height, such as the cell a tap names.
    Under,
}

/// A place's point as the world answers it.
type Point = Answer<Option<Q3>>;

/// The last exact answer of `point`, kept by what it is a function of
/// beside the world: the place and its floor. It is kept until another is
/// asked or the world changes in the rect it read (`change::Memo`): whoever
/// follows a place asks for the same one on every frame. A coarser answer
/// or a lack is not kept: the place is asked again until its leaves come.
static POINT: Global<Memo<(Q3, Floor), Point>> = Global::new();

/// Drops the kept answer of `point` where a change touched the rect it
/// read. It runs on every frame, so each change is seen exactly once
/// (`change::within`).
pub(crate) fn forget_touched() {
    POINT.get(|| Memo::NONE).forget_touched();
}

/// The point `at` is on `floor`, at the leaves: an exact answer of it, a
/// coarser one or a lack where the world does not hold the leaves there, and
/// none where nothing stands.
pub(crate) fn point(at: &Q3, floor: Floor) -> Point {
    let kept = POINT.get(|| Memo::NONE);
    let to = kept.of((*at, floor), || {
        let world = World::now();
        let top = match (floor, is_unknown(at)) {
            (Floor::Under, _) => world.floor_under(at, SizeBy::Leaf),
            (Floor::Ground, true) => world.surface_support(at, SizeBy::Leaf),
            (Floor::Ground, false) => world.floor_nearest(at),
        };
        top.map(|top| top.map(|top| on_floor(at, top)))
    });
    if !matches!(to, Answer::Exact(_)) {
        *kept = Memo::NONE;
    }
    to
}
