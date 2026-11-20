// pxl-core/domain/control/edit/pointer.rs

//! The pointer tool. It points at what the pick struck, as the eraser does
//! (edit.rs `aim`). A tap sends the camera to the cell across the struck
//! face, which is made a point on the floor under it
//! (navigation/place.rs). It previews no edit, and its
//! ghost shows a decor it is over as that decor and anything else as a box,
//! never as a cube.

use crate::domain::control::edit::Hover;
use crate::domain::control::edit::collision::anchor_center;
use crate::domain::control::edit::ghost::{at_corner, decor_of, ghost_of};
use crate::domain::control::navigation;
use crate::domain::preview::Ghost;

pub(super) fn tap(hover: &Hover) {
    let Some(approach) = hover.approach else {
        return;
    };
    navigation::go_to_floor_under(anchor_center(&approach));
}

pub(super) fn ghost(hover: &Hover) -> Ghost {
    if let Some(g) = hover.decor().and_then(ghost_of) {
        return g;
    }
    let Some(h) = hover.coord else {
        return Ghost::None;
    };
    let (object, zoom) = hover.object();
    at_corner(h, zoom, None, false, object.as_ref().and_then(decor_of))
}
