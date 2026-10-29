// pxl-kernel/domain/control/interaction/ghost.rs

//! The ghost the tool held shows under the pointer: the decor a placement
//! would stand or bring back, the box it would fill or carve, or what the
//! eraser is over (`Ghost`, an edit not yet made, beside the others in
//! domain/layer/object/preview.rs). What it looks like, its color, its
//! texture and what stands in for a decor not yet loaded, is the scene's
//! (`domain/scene/indicator/ghost.rs`).

use pxl_spec::asset::cube::{cube_of, Cube};
use pxl_spec::asset::decor::Decor;
use pxl_spec::asset::DecorId;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::control::interaction::edit::{middle, pending, restores_scattered, Pending};
use crate::domain::control::interaction::{holding, hover, ActiveTool};
use crate::domain::layer::object::meta::user_decor_pick;
pub use crate::domain::layer::object::preview::{Ghost, GhostBox};
use crate::domain::layer::object::store::Asset;
use crate::domain::query::changed::Asked;
use crate::domain::query::World;
use xpute_guest::global::Global;

/// The look of the ground the eraser would carve, of the hovered point:
/// asked again only where the hover moved or a change touched its column.
static CARVED: Global<Asked<Q3, Option<Cube>>> = Global::new();

pub fn ghost() -> Ghost {
    let hover = hover();
    let ui = holding();
    let hovered = hover.coord;
    let active_tool = ui.active_tool;
    let Pending {
        object: hovered_object,
        zoom: preview_zoom,
        digging: digging_terrain,
        blocked,
        ..
    } = pending();

    let use_object_style = matches!(
        active_tool,
        ActiveTool::Object | ActiveTool::Decor | ActiveTool::Marker | ActiveTool::Eraser
    ) || hovered_object.is_some();
    let cur_scale = units(if use_object_style {
        preview_zoom
    } else {
        GROUND.cell()
    });

    let carved = CARVED.get(|| Asked::NONE);
    // Looked at each frame turn, a hover not asked of this one too.
    carved.forget_touched();
    // The cell being carved as the world answers it, whichever source fills
    // it: the ground, a way, a bore's wall.
    let carved_style_cube: Option<Cube> = match (&hovered, digging_terrain) {
        (Some(h), true) => carved.of(World::now(), *h, |w| {
            cube_of(
                w.asset_of(&middle(EntryAt::holding(GROUND.cell(), h)))
                    .val()
                    .map_or(0, pxl_spec_core::answer::val),
            )
        }),
        _ => None,
    };

    let cube: Option<Cube> = match (active_tool, hovered_object.map(|o| o.asset())) {
        (ActiveTool::Pointer | ActiveTool::Decor, _) => None,
        (_, Some(Asset::Cube { cube })) => Some(cube),
        (ActiveTool::Object, _) => Some(ui.object_cube),
        _ => carved_style_cube,
    };

    let decor_id = match (active_tool, hovered_object.map(|o| o.asset())) {
        (ActiveTool::Decor, _) => Some(ui.decor_id),
        (_, Some(Asset::Decor { decor_id })) => Some(decor_id),
        _ => None,
    };
    let decor = decor_id.map(Decor::of);

    let decor_corner = match (active_tool, &hovered_object) {
        (ActiveTool::Decor, _) => hovered,
        (_, Some(o)) if matches!(o.asset(), Asset::Decor { .. }) => Some(o.corner()),
        _ => None,
    };
    // Placing back an erased scattered decor restores that instance; the ghost
    // shows it.
    let restored = match (active_tool, &hovered) {
        (ActiveTool::Decor, Some(h)) => restores_scattered(h, ui.decor_id),
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
    let erase = active_tool == ActiveTool::Eraser;
    let scattered_ghost = hover
        .scattered
        .filter(|_| matches!(active_tool, ActiveTool::Pointer | ActiveTool::Eraser))
        .and_then(|s| {
            let (m, decor) = (s.meta, Decor::of(DecorId::of(s.id)?));
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
            let [x, y, z] = crate::domain::view::frame::coord::to_world(&h);
            let c = q3::unpack(&h);
            let box_ghost = GhostBox {
                at: [x, y, z],
                size: cur_scale,
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
