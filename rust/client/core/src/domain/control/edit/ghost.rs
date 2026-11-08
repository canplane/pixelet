// pxl-core/domain/control/edit/ghost.rs

//! The ghost the held tool shows under the pointer, an edit not yet made
//! (`Ghost`, defined with the other previews in domain/preview.rs). It is the
//! decor a placement would stand or bring back, the box it would fill or
//! carve, or what the eraser is over. The scene decides how the ghost looks:
//! its color, its texture and what stands in for a decor not yet loaded
//! (domain/scene/indicator/ghost.rs).

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

/// The palette entry of the ground the eraser would carve at the hovered
/// point. It is asked again only when the hover moves or a change touches
/// its column.
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
        GROUND.leaf_cell_zoom()
    };

    let carved = CARVE_CUBE.get(|| Memo::NONE);
    // This runs on every frame turn, even one whose hover asks nothing of
    // it, so every change is seen once (`changes::within`).
    carved.forget_touched();
    // The cube of the cell being carved, as the world answers it, whichever
    // source fills the cell: the ground, a way or a bore's wall.
    let carved_style_cube: Option<AssetId> = match (&hovered, digging_terrain) {
        (Some(h), true) => carved.of(World::now(), *h, |w| {
            Asset::of(
                w.asset_of(&middle(Octant::holding(GROUND.leaf_cell_zoom(), h)))
                    .val()
                    .map_or(0, answer::val),
            )
            .cube()
        }),
        _ => None,
    };

    // The ghost shows what the tool would place. Otherwise it shows what the
    // pointer or the eraser is over, since a placing tool never hovers an
    // object.
    let shown = ui
        .asset()
        .map(|(a, _)| a)
        .or(hovered_object.map(|o| o.asset()));
    // The pointer shows a decor it is over, but never a cube.
    let cube = shown
        .and_then(Asset::cube)
        .filter(|_| ui != Tool::Pointer)
        .or(carved_style_cube);
    let decor = shown.and_then(Asset::decor);
    let decor_corner = hovered_object.map_or(hovered, |o| Some(o.corner()));
    // Placing an erased scattered decor back restores that same instance, so
    // the ghost shows it.
    let restored = match (ui.asset(), &hovered) {
        (Some((Asset::Decor(id), _)), Some(h)) => restores_scattered(h, id),
        _ => None,
    };
    // Where the decor would stand, and the meta it would carry.
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
        .prop()
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
                // The decor's stand, moved into the world's frame by the same
                // offset as the hover.
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
