// pxl-kernel/layer/marker/hover_cursor.rs

//! The hover cursor's ghost, half transparent: a box in the mesher's layout
//! drawn by the chunk shader, or one decor instance drawn by decor's.

use pxl_render::frame::draw::{DrawList, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR};
use pxl_render::look::style::marker_colors;
use pxl_render::mesh::chunk_mesher::{mesh_words, write_lone_box, Heights, FINE_BOX_WORDS};
use pxl_render::mesh::volume::NO_LAYER;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::ground::shown_ground;
use pxl_spec::bake::pack::channel_of;
use pxl_spec::cell::object::{placeable_cube_num_id, placeable_cube_rgb, terrain_style_cube_for, PlaceableCube};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::{fvec, ivec};
use pxl_spec::space::v3::V3;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free};

use crate::control::interaction::{holding, hover, ActiveTool};
use crate::kernel::with_world;
use crate::layer::decor::asset_store::{decor_assets_for, decor_bundle_assets};
use crate::layer::decor::build::user_decor_pick;
use crate::layer::decor::instance::NO_ENTRY;
use crate::layer::decor::registry::{decor_bundle_for, DECOR_BUNDLES};
use crate::layer::object::collision::{is_placement_blocked, PlacementTool};
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{clear_virtual_carve_preview, get_placed_object, restores_scattered, set_virtual_carve_preview, set_virtual_hide, EntryAt, EntryKind};
use crate::shell::setting::store::settings_store;
use crate::world::access::{Access, FINEST};
use crate::world::change::request_world_tick;
use crate::world::chunk::build::World;
use crate::world::chunk::source::chunk_sources_changed;
use xpute_runtime::global::Global;

#[derive(Clone, Debug, PartialEq)]
enum Ghost {
    None,
    Decor { at: V3, group: u32, rotation: f64 },
    Box { at: V3, size: f64, value: u32, rgb: u32, coplanar: bool },
}

struct Cursor {
    /// The address alone, so re-deriving the same cell does not reset the hide
    /// and force a chunk rebuild.
    virtual_hide_target: Option<EntryAt>,
    carve_preview: Option<(Pvf3, f64)>,
    ghost: Ghost,
}

static CURSOR: Global<Cursor> = Global::new();

pub fn hover_cursor_step() {
    let c = CURSOR.get(|| Cursor {
        virtual_hide_target: None,
        carve_preview: None,
        ghost: Ghost::None,
    });
    let camera = camera_store();
    let ui = holding();
    let settings = settings_store();
    let hovered = camera.hovered_coord;
    let active_tool = ui.active_tool;
    let terrain_carving = settings.terrain_carving;

    let hovered_object = camera.hovered_object_at.and_then(get_placed_object);

    let scattered = camera.hovered_scattered.is_some();
    let digging_terrain = active_tool == ActiveTool::Eraser && hovered_object.is_none() && !scattered;
    let textures = settings.textures;

    let use_object_style = matches!(active_tool, ActiveTool::Object | ActiveTool::Decor | ActiveTool::Eraser) || hovered_object.is_some();

    // A terrain carve is a whole cell: the scan emits nothing for a zero
    // material whatever its sub-cell bytes, so the cell goes whole anyway.
    let preview_size = match (&hovered_object, active_tool) {
        (Some(o), ActiveTool::Eraser | ActiveTool::Pointer) => o.size(),
        (_, ActiveTool::Decor) => DECOR_SIZE,
        _ if digging_terrain => U_PER_CELL,
        _ => ui.object_size,
    };

    let current_scale = if use_object_style { preview_size } else { U_PER_CELL };
    // The same check as the commit, so preview and placement cannot disagree.
    let blocked = match (&hovered, active_tool) {
        (None, _) => false,
        (Some(h), ActiveTool::Object | ActiveTool::Decor) => {
            let tool = if active_tool == ActiveTool::Object { PlacementTool::Object } else { PlacementTool::Decor };
            is_placement_blocked(tool, h, ui.object_size)
        }
        // Eraser hovering an existing object is never blocked; bare terrain is
        // blocked exactly when digging into it isn't enabled.
        (Some(_), ActiveTool::Eraser) => hovered_object.is_none() && !terrain_carving,
        _ => false,
    };

    // The real object's hide lands a tick or more after the ghost, accepted.
    let virtual_hide_target = match (&hovered_object, active_tool) {
        (Some(o), ActiveTool::Eraser) => Some(o.at()),
        _ => None,
    };
    if c.virtual_hide_target != virtual_hide_target {
        set_virtual_hide(virtual_hide_target);
        c.virtual_hide_target = virtual_hide_target;
    }
    set_scattered_hide(camera.hovered_scattered.filter(|_| active_tool == ActiveTool::Eraser).map(|s| s.cell));

    // Bare terrain has no leaf to hide, so the preview injects an AIR entry and
    // what is one voxel below shows through the ghost.
    let carve_preview = hovered.filter(|_| digging_terrain).map(|h| (h, preview_size));
    if c.carve_preview != carve_preview {
        match carve_preview {
            Some((at, size)) => set_virtual_carve_preview(&at, size),
            None => clear_virtual_carve_preview(),
        }
        c.carve_preview = carve_preview;
    }

    let carved_style_cube: Option<PlaceableCube> = match (&hovered, digging_terrain) {
        (Some(h), true) => {
            let cell = Access::now().at(h, FINEST).value().map_or(0, |g| g.cell);
            let shown = shown_ground(|ch| channel_of(cell, ch) as u32);
            terrain_style_cube_for(shown.channel, shown.type_id)
        }
        _ => None,
    };

    let cube: Option<PlaceableCube> = match (active_tool, hovered_object.map(|o| o.kind())) {
        (ActiveTool::Pointer | ActiveTool::Decor, _) => None,
        (_, Some(EntryKind::Solid { cube })) => Some(cube),
        (ActiveTool::Object, _) => Some(ui.object_cube),
        _ => carved_style_cube,
    };

    let decor_id = match (active_tool, hovered_object.map(|o| o.kind())) {
        (ActiveTool::Decor, _) => Some(ui.decor_id),
        (_, Some(EntryKind::Decor { decor_id })) => Some(decor_id),
        _ => None,
    };
    let decor_bundle = decor_id.and_then(decor_bundle_for);
    let decor_assets = decor_assets_for(decor_bundle, decor_bundle_assets());

    let decor_pick_i = match (active_tool, &hovered_object) {
        (ActiveTool::Decor, _) => hovered.map(|h| h.i),
        (_, Some(o)) if matches!(o.kind(), EntryKind::Decor { .. }) => Some(o.corner().i),
        _ => None,
    };
    // Placing back an erased scattered decor restores that instance; the ghost
    // shows it.
    let restored = match (active_tool, &hovered) {
        (ActiveTool::Decor, Some(h)) => restores_scattered(h, DECOR_SIZE, ui.decor_id),
        _ => None,
    };
    let decor_pick = match (decor_bundle, decor_pick_i, restored) {
        (Some(_), _, Some(d)) => Some((d.variant, d.rotation, d.scale)),
        (Some(b), Some(i), None) => {
            let (variant, rotation) = user_decor_pick(i, b.variants.len() as u32);
            Some((variant, rotation, 1.0))
        }
        _ => None,
    };
    // An untextured white silhouette reads as a glitch, so the box ghost stands
    // in until model and skin both arrive.
    let is_decor_ghost = match (decor_assets, decor_pick) {
        (Some(a), Some((variant, _))) => a.models.get(variant as usize).is_some_and(Option::is_some) && a.skins.get(variant as usize).is_some_and(Option::is_some),
        _ => false,
    };

    let swatch = cube.and_then(placeable_cube_rgb);
    let fallback = if active_tool == ActiveTool::Eraser { marker_colors::ERASE } else { marker_colors::ACTION };

    // A carve no longer needs the coplanar bias: its voxel is gone from the mesh
    // while the cursor is on it.
    let coplanar_with_terrain = !use_object_style;

    let scattered_ghost = camera
        .hovered_scattered
        .filter(|_| matches!(active_tool, ActiveTool::Pointer | ActiveTool::Eraser))
        .map(|s| Ghost::Decor {
            at: s.at,
            group: s.group,
            rotation: s.rotation,
            scale: s.scale,
        });
    let ghost = match (scattered_ghost, hovered, blocked) {
        (Some(g), _, _) => g,
        (None, Some(h), false) => {
            let [x, y, z] = projection::unproject(h.i, h.f);
            match (decor_kind, decor_pick) {
                (Some(kind), Some((variant, rotation, scale))) if is_decor_ghost => Ghost::Decor {
                    at: [x + preview_size / 2.0, y + preview_size / 2.0, z],
                    group: kind.first_group() + variant,
                    rotation,
                    scale,
                },
                _ => Ghost::Box {
                    at: [x, y, z],
                    size: current_scale,
                    value: match (cube, textures) {
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
            Ghost::Decor { at, group, rotation } => sched.ghost.set_decor(&mut sched.draw, *at, *group, *rotation as f32),
            Ghost::Box { at, size, value, rgb, coplanar } => sched.ghost.set_box(world, &mut sched.draw, *at, *size, *value, *rgb, *coplanar),
        });
        if written.is_some() {
            c.ghost = ghost;
            request_world_tick();
        }
    }
}

pub const GHOST_OPACITY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Box { size_bits: u64, rgb: [u8; 3], layer: u8, coplanar: bool },
    Decor { group: u32, rotation_bits: u32 },
}

struct Held {
    shape: Shape,
    at: usize,
    bytes: u32,
    handle: u32,
    heights: Heights,
}

#[derive(Default)]
pub struct GhostDraw {
    held: Option<Held>,
    /// A box's min corner, a decor instance's base.
    at: V3,
}

impl GhostDraw {
    /// Painted as `state` paints, or as untextured `rgb` where it paints nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn set_box(&mut self, world: &mut World, draw: &mut DrawList, at: V3, size: f64, state: u32, rgb: u32, coplanar: bool) {
        let (rgb, layer) = match world.painter.material_of(state) {
            Some((m, l)) => ([(m >> 16) as u8, (m >> 8) as u8, m as u8], l),
            None => ([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8], NO_LAYER as u8),
        };
        let shape = Shape::Box {
            size_bits: size.to_bits(),
            rgb,
            layer,
            coplanar,
        };
        self.at = at;
        if self.held.as_ref().is_some_and(|h| h.shape == shape) {
            return;
        }
        let bytes = mesh_words(1, FINE_BOX_WORDS) * 4;
        let Some(block) = self.replace(draw, shape, bytes) else { return };
        // SAFETY: a block of the words asked for, this ghost's until replaced.
        let m = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block), bytes as usize / 4) };
        let heights = write_lone_box(m, size, rgb, layer);
        self.held.as_mut().unwrap().heights = heights;
        self.upload(draw, 1, 0);
    }

    pub fn set_decor(&mut self, draw: &mut DrawList, at: V3, group: u32, rotation: f32, scale: f32) {
        let shape = Shape::Decor {
            group,
            rotation_bits: rotation.to_bits(),
        };
        self.at = at;
        if self.held.as_ref().is_some_and(|h| h.shape == shape) {
            return;
        }
        // One group of one: five float lanes, flags, band, pos, and the index list.
        let bytes = 9 * 4;
        let Some(block) = self.replace(draw, shape, bytes) else { return };
        // SAFETY: as for the box.
        let m = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block), 8) };
        m.copy_from_slice(&[0f32.to_bits(), 0f32.to_bits(), 0f32.to_bits(), rotation.to_bits(), 1f32.to_bits(), 0, NO_ENTRY, 0]);
        self.upload(draw, 0, 0);
    }

    pub fn clear(&mut self, draw: &mut DrawList) {
        if let Some(h) = self.held.take() {
            draw.free(h.handle);
            // SAFETY: the block the ghost took and has not given back.
            unsafe { free((h.at, h.bytes)) };
        }
    }

    fn replace(&mut self, draw: &mut DrawList, shape: Shape, bytes: u32) -> Option<usize> {
        self.clear(draw);
        let (at, bytes) = alloc(bytes)?;
        self.held = Some(Held {
            shape,
            at,
            bytes,
            handle: 0,
            heights: Heights::default(),
        });
        Some(at)
    }

    fn upload(&mut self, draw: &mut DrawList, vertex_cnt: u32, index_cnt: u32) {
        let h = self.held.as_mut().unwrap();
        h.handle = draw.upload(h.at as u32, h.bytes, vertex_cnt, index_cnt);
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        let Some(h) = &self.held else { return };
        let t = DrawList::translation(self.at, eye);
        match h.shape {
            Shape::Box { coplanar, size_bits, .. } => {
                let pipeline = if coplanar { PIPELINE_GHOST_BOX_COPLANAR } else { PIPELINE_GHOST_BOX };
                // Written in a cell of its own size (write_lone_box).
                let unit = f64::from_bits(size_bits) as f32;
                draw.draw_in(pipeline, h.handle, 1, 0, t, GHOST_OPACITY, unit, h.heights.step as f32);
            }
            Shape::Decor { group, .. } => draw.draw_as(PIPELINE_GHOST_DECOR, h.handle, group, 1, t, 0.0, GHOST_OPACITY),
        }
    }
}
