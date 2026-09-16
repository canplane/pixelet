// pxl_core/module/world/marker/hover_cursor.rs

//! The ghost is the kernel's (crates/pxl/core/src/world/ghost.rs): a box written
//! in the mesher's layout and drawn by the chunk shader, or one decor instance
//! drawn by decor's, half transparent. Everything here decides which ghost and
//! where.

use pxl_shared::bake::constant::U_PER_CELL;
use pxl_shared::bake::ground::shown_ground;
use pxl_shared::spec::cell::{placeable_cube_num_id, placeable_cube_rgb, terrain_style_cube_for, PlaceableCube};
use pxl_shared::spec::space::pvf3::fvec;
use pxl_shared::spec::space::pvf3::ivec;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use super::style::marker_colors;
use crate::app::setting::store::use_settings_store;
use crate::app::store::ui_store::{use_ui_store, ActiveTool};
use crate::sys::global::Global;
use crate::sys::kernel::with_world;
use crate::module::camera::store::use_camera_store;
use crate::module::map::store::use_map_store;
use crate::module::world::change::request_world_tick;
use crate::module::world::decor::decor_kinds::user_decor_pick;
use crate::module::world::decor::decor_registry::{decor_bundle_for, DECOR_BUNDLES};
use crate::module::world::decor::asset_store::{decor_assets_for, decor_bundle_assets};
use crate::module::world::frame::coord::get_local_world_origin;
use crate::module::world::object::collision::{is_placement_blocked, PlacementTool};
use crate::module::world::object::size::DECOR_SIZE;
use crate::module::world::object::store::{clear_virtual_carve_preview, get_placed_object, set_virtual_carve_preview, set_virtual_hide, EntryAt, EntryKind};
use crate::module::world::terrain::elevation::get_ground_type;

#[derive(Clone, Debug, PartialEq)]
enum Ghost {
    None,
    Decor { at: [f64; 3], group: u32, rotation: f64 },
    Box { at: [f64; 3], size: f64, value: u32, rgb: u32, coplanar: bool },
}

struct Cursor {
    /// The hide as last set: the address alone, so an unrelated re-derivation
    /// of the same cell doesn't clear-then-reset the hide and force a chunk
    /// rebuild.
    virtual_hide_target: Option<EntryAt>,
    /// The carve preview as last set: its cell and size, compared as words, so
    /// two hover ticks landing in the same cell skip re-running.
    carve_preview: Option<(PVF3, f64)>,
    ghost: Ghost,
}

static CURSOR: Global<Cursor> = Global::new();

/// The hover cursor's turn.
pub fn hover_cursor_step() {
    let c = CURSOR.get(|| Cursor {
        virtual_hide_target: None,
        carve_preview: None,
        ghost: Ghost::None,
    });
    let camera = use_camera_store();
    let ui = use_ui_store();
    let settings = use_settings_store();
    let hovered = camera.hovered_coord;
    let active_tool = ui.active_tool;
    let map_center = use_map_store().map_center;
    let enable_terrain_carving = settings.enable_terrain_carving;

    let hovered_object = camera.hovered_object_at.and_then(get_placed_object);

    // Eraser's own "dig into the terrain itself" case (camera/input.rs's
    // handle_tap/update_hover) — gated on the same settings flag those use, so
    // this ghost never previews an edit the tap handler wouldn't commit.
    let digging_terrain = active_tool == ActiveTool::Eraser && hovered_object.is_none() && enable_terrain_carving;
    let show_structures = settings.show_structures;
    let texture_pack = settings.texture_pack;

    let use_object_style = matches!(active_tool, ActiveTool::Object | ActiveTool::Decor | ActiveTool::Eraser) || hovered_object.is_some();

    // Eraser sizes to whatever's actually hovered when there's a real object
    // under the cursor but falls through to object_size, like the object tool,
    // over bare terrain — its "dig into the terrain itself" case shares the
    // tier picker object placement uses.
    let preview_size = match (&hovered_object, active_tool) {
        (Some(o), ActiveTool::Eraser | ActiveTool::Pointer) => o.size,
        (_, ActiveTool::Decor) => DECOR_SIZE,
        _ => ui.object_size,
    };

    // The ghost box fills [anchor, anchor + current_scale] on every axis — every
    // anchor this file sees is a min corner.
    let current_scale = if use_object_style { preview_size } else { U_PER_CELL };
    // object/collision.rs's is_placement_blocked is the one shared place that
    // decides what counts as blocked, so the preview here and the real commit
    // can't silently disagree.
    let blocked = match (&hovered, active_tool) {
        (None, _) => false,
        (Some(h), ActiveTool::Object | ActiveTool::Decor) => {
            let map_origin = get_local_world_origin(map_center.as_ref());
            let tool = if active_tool == ActiveTool::Object { PlacementTool::Object } else { PlacementTool::Decor };
            is_placement_blocked(tool, h, ui.object_size, map_origin)
        }
        // Eraser hovering an existing object is never blocked; bare terrain is
        // blocked exactly when digging into it isn't enabled.
        (Some(_), ActiveTool::Eraser) => hovered_object.is_none() && !enable_terrain_carving,
        _ => false,
    };

    // Makes the real erase candidate stop rendering, so the ghost reads as a
    // translucent preview over whatever's really behind it. The real object's
    // hide lands on the next build of its chunk, a tick or more after the
    // ghost — accepted as-is.
    let virtual_hide_target = match (&hovered_object, active_tool) {
        (Some(o), ActiveTool::Eraser) => Some(o.at),
        _ => None,
    };
    if c.virtual_hide_target != virtual_hide_target {
        set_virtual_hide(virtual_hide_target);
        c.virtual_hide_target = virtual_hide_target;
    }

    // Terrain's own counterpart: eraser hovering bare terrain has no real leaf
    // to hide, so the carve preview injects a real AIR entry into the build at
    // this hover, so whatever's one voxel below shows through the ghost.
    let carve_preview = hovered.filter(|_| digging_terrain).map(|h| (h, preview_size));
    if c.carve_preview != carve_preview {
        match carve_preview {
            Some((at, size)) => set_virtual_carve_preview(&at, size),
            None => clear_virtual_carve_preview(),
        }
        c.carve_preview = carve_preview;
    }

    // Carving bare terrain previews the terrain voxel it removes, as the cube
    // that looks like one: its terrain-styled id.
    let carved_style_cube: Option<PlaceableCube> = match (&hovered, digging_terrain) {
        (Some(h), true) => {
            let shown = shown_ground(|ch| get_ground_type(h, ch).unwrap_or(0), show_structures);
            terrain_style_cube_for(shown.channel, shown.type_id)
        }
        _ => None,
    };

    // Pointer always reads as the generic marker color; object placement and
    // eraser-on-an-object ghost at the real thing's own colour.
    let cube: Option<PlaceableCube> = match (active_tool, hovered_object.as_ref().map(|o| &o.kind)) {
        (ActiveTool::Pointer | ActiveTool::Decor, _) => None,
        (_, Some(EntryKind::Solid { cube })) => Some(*cube),
        (ActiveTool::Object, _) => Some(ui.object_cube),
        _ => carved_style_cube,
    };

    // Which decor kind's own real silhouette this ghost should show, if any.
    let decor_id = match (active_tool, hovered_object.as_ref().map(|o| &o.kind)) {
        (ActiveTool::Decor, _) => Some(ui.decor_id),
        (_, Some(EntryKind::Decor { decor_id })) => Some(*decor_id),
        _ => None,
    };
    let decor_bundle = decor_id.and_then(decor_bundle_for);
    let decor_assets = decor_assets_for(decor_bundle, decor_bundle_assets());

    // The exact `i` a real placement/hover resolves to — user_decor_pick is pure
    // and keyed only on this, so the ghost predicts the real variant/heading.
    let decor_pick_i = match (active_tool, &hovered_object) {
        (ActiveTool::Decor, _) => hovered.map(|h| h.i),
        (_, Some(o)) if matches!(o.kind, EntryKind::Decor { .. }) => Some(o.i),
        _ => None,
    };
    let decor_pick = match (decor_bundle, decor_pick_i) {
        (Some(b), Some(i)) => Some(user_decor_pick(i, b.variants.len() as u32)),
        _ => None,
    };
    // Both must be ready — an untextured white silhouette would read as a
    // rendering glitch, so this falls back to the box ghost until both resolve.
    let is_decor_ghost = match (decor_assets, decor_pick) {
        (Some(a), Some((variant, _))) => a.models.get(variant as usize).is_some_and(Option::is_some) && a.skins.get(variant as usize).is_some_and(Option::is_some),
        _ => false,
    };

    // Without a texture the ghost falls back to the thing's own swatch. Nothing
    // resolves a swatch only when the eraser is over decor whose model has not
    // arrived: that box takes ERASE, the tool being what has to read off it.
    let swatch = cube.and_then(placeable_cube_rgb);
    let fallback = if active_tool == ActiveTool::Eraser { marker_colors::ERASE } else { marker_colors::ACTION };

    // The pointer's bare-terrain ghost sits flush with real geometry and needs
    // the coplanar bias; a carve no longer does, its voxel being gone from the
    // mesh while the cursor is on it.
    let coplanar_with_terrain = !use_object_style;

    let ghost = match (hovered, blocked) {
        (Some(h), false) => {
            let x = ivec::to_real(ivec::X(h.i)) + fvec::to_real(fvec::X(h.f));
            let y = ivec::to_real(ivec::Y(h.i)) + fvec::to_real(fvec::Y(h.f));
            let z = ivec::to_real(ivec::Z(h.i)) + fvec::to_real(fvec::Z(h.f));
            match (decor_bundle, decor_pick) {
                (Some(bundle), Some((variant, rotation))) if is_decor_ghost => {
                    let before: u32 = DECOR_BUNDLES().iter().take_while(|b| !core::ptr::eq(**b, bundle)).map(|b| b.variants.len() as u32).sum();
                    Ghost::Decor {
                        at: [x + preview_size / 2.0, y + preview_size / 2.0, z],
                        group: before + variant,
                        rotation,
                    }
                }
                _ => Ghost::Box {
                    at: [x, y, z],
                    size: current_scale,
                    value: match (cube, texture_pack) {
                        (Some(cube), true) => placeable_cube_num_id(cube),
                        _ => 0,
                    },
                    rgb: swatch.unwrap_or(fallback),
                    coplanar: coplanar_with_terrain,
                },
            }
        }
        _ => Ghost::None,
    };
    if c.ghost != ghost {
        let written = with_world(|world, sched| match &ghost {
            Ghost::None => sched.ghost.clear(&mut sched.draw),
            Ghost::Decor { at, group, rotation } => sched.ghost.set_decor(world, &mut sched.draw, *at, *group, *rotation as f32),
            Ghost::Box { at, size, value, rgb, coplanar } => sched.ghost.set_box(world, &mut sched.draw, *at, *size, *value, *rgb, *coplanar),
        });
        if written.is_some() {
            c.ghost = ghost;
            request_world_tick();
        }
    }
}
