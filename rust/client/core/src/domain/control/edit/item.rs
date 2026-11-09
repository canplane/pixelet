// pxl-core/domain/control/edit/item.rs

//! An item held to place: a cube, a decor or a marker, at the zoom its scale
//! comes to. It points at the corner flush against the face the pick struck.
//! A tap places it there unless its room is taken, which its preview checks
//! each turn (`blocked`). Its ghost shows it there: a decor as it would
//! stand, or as the instance the ground would bring back, with its box as a
//! stand-in; any other item as its box.

use pxl_spec::space::coord::zoom::Zoom;

use crate::domain::control::edit::collision::{is_placement_blocked, placement_reads};
use crate::domain::control::edit::ghost::{at_corner, stand_at};
use crate::domain::control::edit::pick::Pick;
use crate::domain::control::edit::write::{restores_scattered, Preview};
use crate::domain::control::edit::Hover;
use crate::domain::layer::object::asset::Asset;
use crate::domain::preview::Ghost;
use crate::domain::query::change;
use crate::domain::query::World;

pub(super) fn hover(zoom: Zoom, pick: &Pick) -> Hover {
    Hover {
        coord: Some(pick.adjacent_corner(zoom)),
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

/// `blocked` is asked again only where the item, its zoom, the corner or the
/// store's revision moved, or a change touched the columns the check reads
/// (`placement_reads`, `change::within`), against `was`, the last turn's.
pub(super) fn preview(asset: Asset, zoom: Zoom, hover: &Hover, was: &Preview) -> Preview {
    let Some(h) = hover.coord else {
        return Preview {
            zoom,
            ..Preview::default()
        };
    };
    let blocked_key = Some((asset, zoom, h, World::now().revision()));
    let same =
        was.blocked_key == blocked_key && !change::within(placement_reads(asset, zoom, &h), &[]);
    Preview {
        zoom,
        blocked: if same {
            was.blocked
        } else {
            is_placement_blocked(asset, zoom, &h)
        },
        blocked_key,
        ..Preview::default()
    }
}

pub(super) fn ghost(asset: Asset, hover: &Hover, p: &Preview) -> Ghost {
    let Some(h) = hover.coord.filter(|_| !p.blocked) else {
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
    at_corner(h, p.zoom, asset.cube(), false, decor)
}
