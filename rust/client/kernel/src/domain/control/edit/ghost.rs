// pxl-kernel/domain/control/edit/ghost.rs

//! The ghost the tool held shows under the pointer: the decor a placement
//! would stand or bring back, the box it would fill or carve, or what the
//! eraser is over (`Ghost`, an edit not yet made, beside the others in
//! domain/preview.rs). What it looks like, its color, its
//! texture and what stands in for a decor not yet loaded, is the scene's
//! (`domain/scene/indicator/ghost.rs`).

use crate::domain::layer::object::asset::Asset;
use pxl_spec::asset::AssetId;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;

use crate::domain::control::edit::write::{middle, preview, restores_scattered, Preview};
use crate::domain::control::edit::{hover, tool, Tool};
use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::preview::{Ghost, GhostBox};
use crate::domain::query::changes::Memo;
use crate::domain::query::World;
use crate::domain::view::frame::coord;
use pxl_engine::answer;
use xpute_core::global::Global;

/// The palette entry of the ground the eraser would carve, of the hovered point:
/// asked again only where the hover moved or a change touched its column.
static CARVE_CUBE: Global<Memo<Q3, Option<AssetId>>> = Global::new();

pub fn ghost() -> Ghost {
    let hover = hover();
    let ui = tool();
    let hovered = hover.coord;
    let Preview {
        object: hovered_object,
        zoom: preview_zoom,
        digging: digging_terrain,
        blocked,
        ..
    } = preview();

    let use_object_style = ui != Tool::Pointer || hovered_object.is_some();
    let box_zoom = if use_object_style {
        preview_zoom
    } else {
        GROUND.cell()
    };

    let carved = CARVE_CUBE.get(|| Memo::NONE);
    // Looked at each frame turn, a hover not asked of this one too.
    carved.forget_touched();
    // The cell being carved as the world answers it, whichever source fills
    // it: the ground, a way, a bore's wall.
    let carved_style_cube: Option<AssetId> = match (&hovered, digging_terrain) {
        (Some(h), true) => carved.of(World::now(), *h, |w| {
            Asset::of(
                w.asset_of(&middle(Octant::holding(GROUND.cell(), h)))
                    .val()
                    .map_or(0, answer::val),
            )
            .cube()
        }),
        _ => None,
    };

    // What it shows: what the tool would place; else, as a placing tool hovers
    // no object, what the pointer or the eraser is over.
    let shown = ui
        .asset()
        .map(|(a, _)| a)
        .or(hovered_object.map(|o| o.asset()));
    // The pointer shows a decor it is over, and no cube.
    let cube = shown
        .and_then(Asset::cube)
        .filter(|_| ui != Tool::Pointer)
        .or(carved_style_cube);
    let decor = shown.and_then(Asset::decor);
    let decor_corner = hovered_object.map_or(hovered, |o| Some(o.corner()));
    // Placing back an erased scattered decor restores that instance; the ghost
    // shows it.
    let restored = match (ui.asset(), &hovered) {
        (Some((Asset::Decor(id), _)), Some(h)) => restores_scattered(h, id),
        _ => None,
    };
    // Where it would stand, and what it would carry.
    let decor_pick = match (decor, decor_corner, restored) {
        (Some(_), _, Some(d)) => Some((d.at, d.meta)),
        (Some(b), Some(c), None) => {
            let (variant, turn) = user_decor_pick(&c, b);
            Some(b.stand(q3::unpack(&c), variant, turn, 1.0))
        }
        _ => None,
    };
    let erase = ui == Tool::Eraser;
    let scattered_ghost = hover
        .scattered_decor()
        .filter(|_| matches!(ui, Tool::Pointer | Tool::Eraser))
        .and_then(|s| {
            let (m, decor) = (s.meta, Asset::of(u32::from(s.id)).decor()?);
            Some(Ghost::Decor {
                at: s.at,
                decor,
                variant: m.variant,
                rot: m.spin(),
                scale: m.scale,
                stand_in: None,
            })
        });
    match (scattered_ghost, hovered, blocked) {
        (Some(g), _, _) => g,
        (None, Some(h), false) => {
            let [x, y, z] = coord::to_world(&h);
            let c = q3::unpack(&h);
            let box_ghost = GhostBox {
                at: [x, y, z],
                zoom: box_zoom,
                cube,
                erase,
            };
            match (decor, decor_pick) {
                // Its stand, moved as the hover is into the world's frame.
                (Some(decor), Some((at, m))) => Ghost::Decor {
                    at: [at[0] - c[0] + x, at[1] - c[1] + y, z],
                    decor,
                    variant: m.variant,
                    rot: m.spin(),
                    scale: m.scale,
                    stand_in: Some(box_ghost),
                },
                _ => Ghost::Box(box_ghost),
            }
        }
        _ => Ghost::None,
    }
}
