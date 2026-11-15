// pxl-core/domain/control/edit/eraser.rs

//! The eraser. It points at what the pick struck, as the pointer does
//! (edit.rs `aim`). A tap erases the placed object struck, the generated
//! decor's leaf cell, or else a leaf cell of the ground. Its preview hides
//! what it is over in the world as drawn, or cuts bare ground to air, and its
//! ghost marks the box it would take, of the cube it takes.

use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;

use crate::domain::control::edit::Hover;
use crate::domain::control::edit::ghost::{at_corner, carved_cube, decor_of, ghost_of};
use crate::domain::control::edit::write::{place_air, remove};
use crate::domain::preview::{Ghost, RenderEdit};

pub(super) fn tap(hover: &Hover) {
    let Some(h) = hover.coord else {
        return;
    };
    // Where nothing was struck it takes a whole leaf cell of the ground,
    // never the size tier.
    let at = hover
        .target
        .map_or(Octant::holding(GROUND.leaf_cell_zoom(), &h), |t| t.at);
    // Only what is placed has an entry to take. Over what the base holds,
    // Air hides it, and a marker sharing a generated decor's cell goes with it.
    match hover.object().0 {
        Some(_) => remove(at),
        None => place_air(at),
    }
}

pub(super) fn render_edit(hover: &Hover) -> RenderEdit {
    let (object, zoom) = hover.object();
    let generated = hover.decor();
    RenderEdit {
        hide: object.map(|o| o.id()),
        // Over bare terrain, an air entry that is never written goes into the
        // drawn build, so what lies one voxel below shows through the ghost.
        carve: hover
            .coord
            .filter(|_| object.is_none() && generated.is_none())
            .map(|h| Octant::holding(zoom, &h)),
        scattered_cell: generated.and_then(|s| s.generated_in()),
    }
}

pub(super) fn ghost(hover: &Hover) -> Ghost {
    if let Some(g) = hover.decor().and_then(ghost_of) {
        return g;
    }
    let Some(h) = hover.coord else {
        return Ghost::None;
    };
    let (object, zoom) = hover.object();
    let cube = match object {
        Some(o) => o.asset().cube(),
        None => carved_cube(h),
    };
    at_corner(h, zoom, cube, true, object.as_ref().and_then(decor_of))
}
