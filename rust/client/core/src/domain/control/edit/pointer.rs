// pxl-core/domain/control/edit/pointer.rs

//! The pointer tool. It points at what the pick struck, as the eraser does
//! (edit.rs `aim`). A tap sends the pivot to the cell across the struck face,
//! where it stands as it does where a pan lands. It previews no edit, and its
//! ghost shows a decor it is over as that decor and anything else as a box,
//! never as a cube.

use crate::domain::control::camera::controller::Motion;
use crate::domain::control::edit::collision::anchor_center;
use crate::domain::control::edit::ghost::{at_corner, decor_of, prop};
use crate::domain::control::edit::write::Preview;
use crate::domain::control::edit::Hover;
use crate::domain::control::navigation::destination;
use crate::domain::control::support::stand;
use crate::domain::preview::Ghost;

pub(super) fn tap(hover: &Hover) {
    let Some(approach) = hover.approach else {
        return;
    };
    destination::go_to(stand(&anchor_center(&approach)), Motion::Smooth);
}

pub(super) fn preview(hover: &Hover) -> Preview {
    let (object, zoom) = hover.object();
    Preview {
        object,
        zoom,
        ..Preview::default()
    }
}

pub(super) fn ghost(hover: &Hover, p: &Preview) -> Ghost {
    if let Some(g) = hover.prop().and_then(prop) {
        return g;
    }
    let Some(h) = hover.coord else {
        return Ghost::None;
    };
    at_corner(h, p.zoom, None, false, p.object.as_ref().and_then(decor_of))
}
