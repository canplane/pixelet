// pxl-core/domain/control/edit/ghost.rs

//! The ghost the held tool shows under the pointer, an edit not yet made
//! (`Ghost`, defined with the other previews in domain/preview.rs). Each tool
//! decides its own (pointer.rs, eraser.rs, item.rs) from the pieces here: a
//! generated decor shown as itself, a placed decor as it stands, the box at
//! the hovered corner, and the cube of the ground the eraser would carve. The
//! scene decides how the ghost looks: its color, its texture and what stands
//! in for a decor not yet loaded (domain/scene/indicator/ghost.rs).

use pxl_spec::asset::decor::{Decor, DecorMeta};
use pxl_spec::asset::AssetId;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;

use crate::domain::control::edit::write::{middle, preview};
use crate::domain::control::edit::{hover, tool};
use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::asset::Asset;
use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::layer::object::store::Entry;
use crate::domain::preview::{Ghost, GhostBox};
use crate::domain::query::changes::Memo;
use crate::domain::query::World;
use crate::domain::view::frame::coord;
use pxl_engine::answer;
use xpute_core::global::Global;

/// The cube of the ground the eraser would carve at the hovered point. It is
/// asked again only when the hover moves or a change touches its column.
static CARVE_CUBE: Global<Memo<Q3, Option<AssetId>>> = Global::new();

fn carved() -> &'static mut Memo<Q3, Option<AssetId>> {
    CARVE_CUBE.get(|| Memo::NONE)
}

pub fn ghost() -> Ghost {
    // This runs on every frame turn, whichever tool is held, so every change
    // is seen once (`changes::within`).
    carved().forget_touched();
    tool().ghost(&hover(), &preview())
}

/// The cube of the leaf cell at `h` that a carve takes, as the world answers
/// it, whichever source fills the cell: the ground, a way or a bore's wall.
pub(super) fn carved_cube(h: Q3) -> Option<AssetId> {
    carved().of(World::now(), h, |w| {
        Asset::of(
            w.asset_of(&middle(Octant::holding(GROUND.leaf_cell_zoom(), &h)))
                .val()
                .map_or(0, answer::val),
        )
        .cube()
    })
}

/// A generated decor shown as itself, where it stands.
pub(super) fn prop(i: Instance) -> Option<Ghost> {
    let (m, decor) = (i.meta, Asset::of(u32::from(i.id)).decor()?);
    Some(Ghost::Decor {
        at: i.at,
        decor,
        variant: m.variant,
        rot: m.spin(),
        scale: m.scale,
        stand_in: None,
    })
}

/// How `d` placed with its box's corner at `corner` stands: the variant and
/// heading its corner picks (`user_decor_pick`).
pub(super) fn stand_at(corner: &Q3, d: &Decor) -> ([f64; 3], DecorMeta) {
    let (variant, turn) = user_decor_pick(corner, d);
    d.stand(q3::unpack(corner), variant, turn, 1.0)
}

/// The decor a placed object is, standing as it stands.
pub(super) fn decor_of(o: &Entry) -> Option<(&'static Decor, ([f64; 3], DecorMeta))> {
    let d = o.asset().decor()?;
    Some((d, stand_at(&o.corner(), d)))
}

/// The ghost at the hovered corner `h`: the box of `zoom` it would fill or
/// take, of `cube`, the eraser's where `erase`; or a decor standing as
/// `decor` gives, with that box as its stand-in.
pub(super) fn at_corner(
    h: Q3,
    zoom: Zoom,
    cube: Option<AssetId>,
    erase: bool,
    decor: Option<(&'static Decor, ([f64; 3], DecorMeta))>,
) -> Ghost {
    let [x, y, z] = coord::to_world(&h);
    let c = q3::unpack(&h);
    let b = GhostBox {
        at: [x, y, z],
        zoom,
        cube,
        erase,
    };
    match decor {
        // The decor's stand, moved into the world's frame by the same offset
        // as the hover.
        Some((decor, (at, m))) => Ghost::Decor {
            at: [at[0] - c[0] + x, at[1] - c[1] + y, z],
            decor,
            variant: m.variant,
            rot: m.spin(),
            scale: m.scale,
            stand_in: Some(b),
        },
        None => Ghost::Box(b),
    }
}
