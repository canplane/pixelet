// pxl-core/domain/control/edit/eraser.rs

//! The eraser. It points at what the pick struck, as the pointer does
//! (edit.rs `aim`). A tap erases the placed object struck, the generated
//! decor's leaf cell, or else a leaf cell of the ground. Its preview hides
//! what it is over in the world as drawn, or cuts bare ground to air, and its
//! ghost marks the box it would take, of the cube it takes.

use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;

use crate::domain::control::edit::ghost::{at_corner, carved_cube, decor_of, prop};
use crate::domain::control::edit::write::{place_air, remove, Preview};
use crate::domain::control::edit::{Hover, Target};
use crate::domain::preview::{Ghost, RenderEdit};

pub(super) fn tap(hover: &Hover) {
    let Some(h) = hover.coord else {
        return;
    };
    match hover.target {
        Some(Target::Object(at)) => remove(at),
        Some(Target::Prop(cell, _)) => place_air(Octant::new(GROUND.leaf_cell_zoom(), cell)),
        // Terrain has no entry to delete, so the eraser writes AIR over it
        // rather than VOID. It writes a whole leaf cell, never the size tier.
        None => place_air(Octant::holding(GROUND.leaf_cell_zoom(), &h)),
    }
}

pub(super) fn preview(hover: &Hover) -> Preview {
    let (object, zoom) = hover.object();
    let prop = hover.prop();
    let edit = RenderEdit {
        hide: object.map(|o| o.id()),
        // Over bare terrain, an AIR entry that is never written goes into the
        // drawn build, so what lies one voxel below shows through the ghost.
        carve: hover
            .coord
            .filter(|_| object.is_none() && prop.is_none())
            .map(|h| Octant::holding(zoom, &h)),
        scattered_cell: prop.and_then(|s| s.generated_in()),
    };
    Preview {
        object,
        zoom,
        edit,
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
    let cube = match p.object {
        Some(o) => o.asset().cube(),
        None => carved_cube(h),
    };
    at_corner(h, p.zoom, cube, true, p.object.as_ref().and_then(decor_of))
}
