// pxl-kernel/scene/indicator/hover_cursor.rs

//! The hover cursor's ghost, half transparent: a box in the mesher's layout
//! drawn by the chunk shader, or one decor instance drawn by decor's. What it
//! shows is the tool's (`control/interaction/ghost.rs`); this draws it.

use pxl_render::frame::draw::{DrawList, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR};
use pxl_render::mesh::chunk::{on_whole_units, write_lone_box, Heights, FINE_BOX_WORDS};
use pxl_render::mesh::paint::object::PAINTER;
use pxl_render::mesh::word::NO_LAYER;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};

pub const GHOST_OPACITY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Box {
        size_bits: u64,
        offset_bits: [u64; 3],
        rgb: [u8; 3],
        layer: u8,
        coplanar: bool,
    },
    Decor {
        group: u32,
        rotation_bits: u32,
        scale_bits: u32,
    },
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
    pub fn set_box(&mut self, draw: &mut DrawList, at: V3, size: f64, state: u32, rgb: u32, coplanar: bool) -> bool {
        let (rgb, layer) = PAINTER.look_of(state).unwrap_or(([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8], NO_LAYER as u8));
        let offset = on_whole_units(at).1;
        let shape = Shape::Box {
            size_bits: size.to_bits(),
            offset_bits: offset.map(f64::to_bits),
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
        let heights = write_lone_box(m, offset, size, rgb, layer);
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
        // One group of one: five float lanes, its flags, and the index list.
        let bytes = 7 * 4;
        let Some(block) = self.replace(draw, shape, bytes) else { return true };
        // SAFETY: as for the box.
        let m = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block), 7) };
        m.copy_from_slice(&[0f32.to_bits(), 0f32.to_bits(), 0f32.to_bits(), rotation.to_bits(), scale.to_bits(), 0, 0]);
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
        match h.shape {
            Shape::Box { coplanar, size_bits, .. } => {
                let pipeline = if coplanar { PIPELINE_GHOST_BOX_COPLANAR } else { PIPELINE_GHOST_BOX };
                let t = DrawList::translation(on_whole_units(self.at).0, eye);
                // Written in a cell of its own size (write_lone_box).
                let unit = f64::from_bits(size_bits) as f32;
                draw.draw_in(pipeline, h.handle, 1, 0, t, GHOST_OPACITY, unit, h.heights.step as f32);
            }
            Shape::Decor { group, .. } => draw.draw_instances(PIPELINE_GHOST_DECOR, h.handle, group, 1, [0, 1, 6], DrawList::translation(self.at, eye), GHOST_OPACITY),
        }
    }
}
