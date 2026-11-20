// pxl-core/domain/control/edit/item.rs

//! An item held to place: a cube, a decor or a marker, at the zoom its scale
//! comes to. It points at the corner flush against the face the pick struck.
//! A tap places it there unless its room is taken, and its ghost shows it
//! only where the tap would: a decor as it would stand, or as the instance
//! the ground would bring back, with its box as a stand-in; any other item as
//! its box.

use pxl_spec::space::coord::zoom::Zoom;

use crate::domain::control::edit::Hover;
use crate::domain::control::edit::collision::is_placement_blocked;
use crate::domain::control::edit::ghost::{at_corner, stand_at};
use crate::domain::control::edit::pick::Pick;
use crate::domain::control::edit::write::restores_scattered;
use crate::domain::layer::object::asset::Asset;
use crate::domain::preview::Ghost;

/// An item has a place only where the eye draws cells of its size: where it
/// draws coarser ones, the item would be folded into a summary the pointer
/// cannot strike (pick.rs `Pick::is_folded`).
pub(super) fn hover(zoom: Zoom, pick: &Pick) -> Hover {
    Hover {
        coord: pick.draws(zoom).then(|| pick.adjacent_corner(zoom)),
        target: None,
        approach: None,
    }
}

pub(super) fn tap(asset: Asset, zoom: Zoom, hover: &Hover) {
    let Some(h) = hover.coord else {
        return;
    };
    // The room may have filled since the hover was last updated.
    if !is_placement_blocked(asset, zoom, &h) {
        asset.place(&h, zoom);
    }
}

pub(super) fn ghost(asset: Asset, zoom: Zoom, hover: &Hover) -> Ghost {
    let Some(h) = hover
        .coord
        .filter(|h| !is_placement_blocked(asset, zoom, h))
    else {
        return Ghost::None;
    };
    let decor = asset.decor().map(|d| {
        // Placing an erased generated decor back restores that same
        // instance, so the ghost shows it.
        let restored = restores_scattered(&h, d.id);
        (
            d,
            restored.map_or_else(|| stand_at(&h, d), |i| (i.at, i.meta)),
        )
    });
    at_corner(h, zoom, asset.cube(), false, decor)
}
