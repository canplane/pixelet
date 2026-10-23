// pxl-kernel/control/interaction/ghost.rs

//! The ghost the tool held shows under the pointer: the decor a placement
//! would stand or bring back, the box it would fill or carve, or what the
//! eraser is over.

use pxl_render::look::color::indicator;
use pxl_spec::block::decor::Decor;
use pxl_spec::block::object::{cube_id, cube_of, cube_rgb, Cube};
use pxl_spec::block::DecorId;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::control::interaction::edit::{pending, restores_scattered, Pending};
use crate::control::interaction::{holding, hover, ActiveTool};
use crate::decor::assets::decor_assets_for;
use crate::layer::instance::Meta;
use crate::layer::object::meta::user_decor_pick;
use crate::layer::object::store::Block;
use crate::query::World;
use crate::setting::store::settings_store;

/// What the pointer's tool would show where it is: nothing, a decor
/// instance, or a box, drawn half transparent by the scene.
pub enum Ghost {
    None,
    Decor { at: V3, group: u32, rotation: f64, scale: f64 },
    Box { at: V3, size: f64, value: u32, rgb: u32, coplanar: bool },
}

pub fn ghost() -> Ghost {
    let hover = hover();
    let ui = holding();
    let settings = settings_store();
    let hovered = hover.coord;
    let active_tool = ui.active_tool;
    let Pending {
        object: hovered_object,
        size: preview_size,
        digging: digging_terrain,
        blocked,
        ..
    } = pending();
    let textures = settings.textures;

    let use_object_style = matches!(active_tool, ActiveTool::Object | ActiveTool::Decor | ActiveTool::Marker | ActiveTool::Eraser) || hovered_object.is_some();
    let current_scale = if use_object_style { preview_size } else { units(GROUND.cell()) };

    let carved_style_cube: Option<Cube> = match (&hovered, digging_terrain) {
        (Some(h), true) => {
            let cell = World::now().surface(h, LEAF_ZOOM).value().flatten().map_or(0, |s| s.block);
            cube_of(u32::from(cell))
        }
        _ => None,
    };

    let cube: Option<Cube> = match (active_tool, hovered_object.map(|o| o.block())) {
        (ActiveTool::Pointer | ActiveTool::Decor, _) => None,
        (_, Some(Block::Cube { cube })) => Some(cube),
        (ActiveTool::Object, _) => Some(ui.object_cube),
        _ => carved_style_cube,
    };

    let decor_id = match (active_tool, hovered_object.map(|o| o.block())) {
        (ActiveTool::Decor, _) => Some(ui.decor_id),
        (_, Some(Block::Decor { decor_id })) => Some(decor_id),
        _ => None,
    };
    let decor = decor_id.map(Decor::of);
    let decor_assets = decor.and_then(decor_assets_for);

    let decor_corner = match (active_tool, &hovered_object) {
        (ActiveTool::Decor, _) => hovered,
        (_, Some(o)) if matches!(o.block(), Block::Decor { .. }) => Some(o.corner()),
        _ => None,
    };
    // Placing back an erased scattered decor restores that instance; the ghost
    // shows it.
    let restored = match (active_tool, &hovered) {
        (ActiveTool::Decor, Some(h)) => restores_scattered(h, units(Decor::of(ui.decor_id).row.cell_zoom()), ui.decor_id),
        _ => None,
    };
    // Where it would stand, and what it would carry.
    let decor_pick = match (decor, decor_corner, restored) {
        (Some(_), _, Some(d)) => {
            let Meta::Decor(m) = d.meta;
            Some((d.at, m))
        }
        (Some(b), Some(c), None) => {
            let (variant, turn) = user_decor_pick(&c, b);
            Some(b.stand(real(&c), variant, turn, 1.0))
        }
        _ => None,
    };
    // An untextured white silhouette reads as a glitch, so the box ghost stands
    // in until model and skin both arrive.
    let is_decor_ghost = match (decor_assets, decor_pick) {
        (Some(a), Some((_, m))) => a.models.get(m.variant as usize).is_some_and(Option::is_some) && a.skin(m.variant as usize).is_some(),
        _ => false,
    };

    let swatch = cube.and_then(cube_rgb);
    let fallback = if active_tool == ActiveTool::Eraser { indicator::ERASE } else { indicator::ACTION };

    // A carve no longer needs the coplanar bias: its voxel is gone from the mesh
    // while the cursor is on it.
    let coplanar_with_terrain = !use_object_style;

    let scattered_ghost = hover.scattered.filter(|_| matches!(active_tool, ActiveTool::Pointer | ActiveTool::Eraser)).and_then(|s| {
        let (Meta::Decor(m), decor) = (s.meta, Decor::of(DecorId::of(s.id)?));
        Some(Ghost::Decor {
            at: s.at,
            group: decor.first_group() + m.variant,
            rotation: m.spin(),
            scale: m.scale,
        })
    });
    match (scattered_ghost, hovered, blocked) {
        (Some(g), _, _) => g,
        (None, Some(h), false) => {
            let [x, y, z] = crate::view::frame::coord::to_world(&h);
            let c = real(&h);
            match (decor, decor_pick) {
                // Its stand, moved as the hover is into the world's frame.
                (Some(decor), Some((at, m))) if is_decor_ghost => Ghost::Decor {
                    at: [at[0] - c[0] + x, at[1] - c[1] + y, z],
                    group: decor.first_group() + m.variant,
                    rotation: m.spin(),
                    scale: m.scale,
                },
                _ => Ghost::Box {
                    at: [x, y, z],
                    size: current_scale,
                    value: match (cube, textures) {
                        (Some(cube), true) => cube_id(cube),
                        _ => 0,
                    },
                    rgb: swatch.unwrap_or(fallback),
                    coplanar: coplanar_with_terrain,
                },
            }
        }
        _ => Ghost::None,
    }
}

/// `p` in units, in the canonical frame.
fn real(p: &Q3) -> V3 {
    [q3::to_real(i64::from(p[0])), q3::to_real(i64::from(p[1])), q3::to_real(i64::from(p[2]))]
}
