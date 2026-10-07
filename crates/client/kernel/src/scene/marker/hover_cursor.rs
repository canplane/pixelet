// pxl-kernel/scene/marker/hover_cursor.rs

//! The hover cursor's ghost, half transparent: a box in the mesher's layout
//! drawn by the chunk shader, or one decor instance drawn by decor's.

use pxl_render::frame::draw::{DrawList, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR};
use pxl_render::look::color::marker;
use pxl_render::mesh::chunk::{write_lone_box, Heights, FINE_BOX_WORDS};
use pxl_render::mesh::word::NO_LAYER;
use pxl_spec::cell::object::{placeable_cube_num_id, placeable_cube_of, placeable_cube_rgb, PlaceableCube};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};

use crate::chunk::decor::build::user_decor_pick;
use crate::chunk::decor::instance::NO_ENTRY;
use crate::chunk::decor::kind::kind_for;
use crate::chunk::decor::kind_assets::decor_assets_for;
use crate::control::interaction::edit::{pending, restores_scattered, Pending};
use crate::control::interaction::{holding, hover, ActiveTool};
use crate::kernel;
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::EntryKind;
use crate::query::World;
use crate::sched::edge::request_world_tick;
use crate::setting::store::settings_store;
use crate::Build;

enum Ghost {
    None,
    Decor { at: V3, group: u32, rotation: f64, scale: f64 },
    Box { at: V3, size: f64, value: u32, rgb: u32, coplanar: bool },
}

pub fn hover_cursor_step() {
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

    let use_object_style = matches!(active_tool, ActiveTool::Object | ActiveTool::Decor | ActiveTool::Eraser) || hovered_object.is_some();
    let current_scale = if use_object_style { preview_size } else { units(GROUND.cell()) };

    let carved_style_cube: Option<PlaceableCube> = match (&hovered, digging_terrain) {
        (Some(h), true) => {
            let cell = World::now().at(h, LEAF_ZOOM).value().map_or(0, |g| g.cell);
            placeable_cube_of(u32::from(cell))
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
    let decor_kind = decor_id.and_then(kind_for);
    let decor_assets = decor_kind.and_then(decor_assets_for);

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
    let decor_pick = match (decor_kind, decor_pick_i, restored) {
        (Some(_), _, Some(d)) => Some((d.variant, d.rotation, d.scale)),
        (Some(b), Some(i), None) => {
            let (variant, rotation) = user_decor_pick(i, b.variant_count());
            Some((variant, rotation, 1.0))
        }
        _ => None,
    };
    // An untextured white silhouette reads as a glitch, so the box ghost stands
    // in until model and skin both arrive.
    let is_decor_ghost = match (decor_assets, decor_pick) {
        (Some(a), Some((variant, _, _))) => a.models.get(variant as usize).is_some_and(Option::is_some) && a.skin(variant as usize).is_some(),
        _ => false,
    };

    let swatch = cube.and_then(placeable_cube_rgb);
    let fallback = if active_tool == ActiveTool::Eraser { marker::ERASE } else { marker::ACTION };

    // A carve no longer needs the coplanar bias: its voxel is gone from the mesh
    // while the cursor is on it.
    let coplanar_with_terrain = !use_object_style;

    let scattered_ghost = hover.scattered.filter(|_| matches!(active_tool, ActiveTool::Pointer | ActiveTool::Eraser)).map(|s| Ghost::Decor {
        at: s.at,
        group: s.group,
        rotation: s.rotation,
        scale: s.scale,
    });
    let ghost = match (scattered_ghost, hovered, blocked) {
        (Some(g), _, _) => g,
        (None, Some(h), false) => {
            let [x, y, z] = crate::view::frame::coord::to_world(&h);
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
    let changed = kernel().parts().map(|(mut build, _, scene, draw)| match ghost {
        Ghost::None => scene.ghost.clear(draw),
        Ghost::Decor { at, group, rotation, scale } => scene.ghost.set_decor(draw, at, group, rotation as f32, scale as f32),
        Ghost::Box { at, size, value, rgb, coplanar } => scene.ghost.set_box(&mut build, draw, at, size, value, rgb, coplanar),
    });
    if changed == Some(true) {
        request_world_tick();
    }
}

pub const GHOST_OPACITY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Box { size_bits: u64, rgb: [u8; 3], layer: u8, coplanar: bool },
    Decor { group: u32, rotation_bits: u32, scale_bits: u32 },
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
    /// Painted as `state` paints, or as untextured `rgb` where it paints
    /// nothing. Whether what is drawn changed.
    #[allow(clippy::too_many_arguments)]
    pub fn set_box(&mut self, build: &mut Build, draw: &mut DrawList, at: V3, size: f64, state: u32, rgb: u32, coplanar: bool) -> bool {
        let (rgb, layer) = match build.painter.material_of(state) {
            Some((m, l)) => ([(m >> 16) as u8, (m >> 8) as u8, m as u8], l),
            None => ([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8], NO_LAYER as u8),
        };
        let shape = Shape::Box {
            size_bits: size.to_bits(),
            rgb,
            layer,
            coplanar,
        };
        let moved = core::mem::replace(&mut self.at, at) != at;
        if self.held.as_ref().is_some_and(|h| h.shape == shape) {
            return moved;
        }
        let bytes = FINE_BOX_WORDS * 4;
        let Some(block) = self.replace(draw, shape, bytes) else { return true };
        // SAFETY: a block of the words asked for, this ghost's until replaced.
        let m = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block), bytes as usize / 4) };
        let heights = write_lone_box(m, size, rgb, layer);
        self.held.as_mut().unwrap().heights = heights;
        self.upload(draw, 1, 0);
        true
    }

    pub fn set_decor(&mut self, draw: &mut DrawList, at: V3, group: u32, rotation: f32, scale: f32) -> bool {
        let shape = Shape::Decor {
            group,
            rotation_bits: rotation.to_bits(),
            scale_bits: scale.to_bits(),
        };
        let moved = core::mem::replace(&mut self.at, at) != at;
        if self.held.as_ref().is_some_and(|h| h.shape == shape) {
            return moved;
        }
        // One group of one: five float lanes, flags, band, pos, and the index list.
        let bytes = 9 * 4;
        let Some(block) = self.replace(draw, shape, bytes) else { return true };
        // SAFETY: as for the box.
        let m = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block), 9) };
        m.copy_from_slice(&[0f32.to_bits(), 0f32.to_bits(), 0f32.to_bits(), rotation.to_bits(), scale.to_bits(), 0, NO_ENTRY, 0, 0]);
        self.upload(draw, 0, 0);
        true
    }

    /// Whether there was one.
    pub fn clear(&mut self, draw: &mut DrawList) -> bool {
        let Some(h) = self.held.take() else { return false };
        draw.free(h.handle);
        // SAFETY: the block the ghost took and has not given back.
        unsafe { free((h.at, h.bytes)) };
        true
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
            Shape::Decor { group, .. } => draw.draw_instances(PIPELINE_GHOST_DECOR, h.handle, group, 1, [0, 1, 8], t, GHOST_OPACITY),
        }
    }
}
