// pxl-kernel/scene/indicator/hover_cursor.rs

//! The hover cursor's ghost, half transparent: a box the box shader unfolds
//! (pxl-render `mesh::ghost`), or one decor instance drawn by decor's. What it
//! shows is the tool's (`control/interaction/ghost.rs`); this draws it.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR};
use pxl_render::frame::shader::record::BOX_WORDS;
use pxl_render::frame::shader::words::NO_LAYER;
use pxl_render::gpu::api::bytes;
use pxl_render::mesh::ghost::{on_whole_units, write_lone_box};
use pxl_render::mesh::paint::object::PAINTER;
use pxl_spec::space::coord::v3::V3;

pub const GHOST_OPACITY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Box {
        size_bits: u64,
        offset_bits: [u64; 3],
        rgb: [u8; 3],
        layer: u8,
    },
    Decor {
        group: u32,
        rotation_bits: u32,
        scale_bits: u32,
    },
}

#[derive(Default)]
pub struct GhostDraw {
    /// What `held` holds; none where nothing is drawn.
    shape: Option<Shape>,
    held: Held,
    /// A box's, as written (write_lone_box).
    z_step: f64,
    /// A box's min corner, a decor instance's base.
    at: V3,
}

impl GhostDraw {
    /// Painted as `state` paints, or as untextured `rgb` where it paints
    /// nothing. Whether what is drawn changed.
    pub fn set_box(&mut self, draw: &mut DrawList, at: V3, size: f64, state: u32, rgb: u32) -> bool {
        let (rgb, layer) = PAINTER.look_of(state).unwrap_or(([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8], NO_LAYER as u8));
        let offset = on_whole_units(at).1;
        let shape = Shape::Box {
            size_bits: size.to_bits(),
            offset_bits: offset.map(f64::to_bits),
            rgb,
            layer,
        };
        let moved = core::mem::replace(&mut self.at, at) != at;
        if self.shape == Some(shape) {
            return moved;
        }
        let mut m = [0; BOX_WORDS as usize];
        self.z_step = write_lone_box(&mut m, offset, size, rgb, layer);
        self.write(draw, shape, bytes(&m));
        true
    }

    pub fn set_decor(&mut self, draw: &mut DrawList, at: V3, group: u32, rotation: f32, scale: f32) -> bool {
        let shape = Shape::Decor {
            group,
            rotation_bits: rotation.to_bits(),
            scale_bits: scale.to_bits(),
        };
        let moved = core::mem::replace(&mut self.at, at) != at;
        if self.shape == Some(shape) {
            return moved;
        }
        // One group of one: five float lanes, its flags, and the index list.
        let m = [0f32.to_bits(), 0f32.to_bits(), 0f32.to_bits(), rotation.to_bits(), scale.to_bits(), 0, 0];
        self.write(draw, shape, bytes(&m));
        true
    }

    /// Whether there was one.
    pub fn clear(&mut self, draw: &mut DrawList) -> bool {
        self.held.free(draw);
        self.shape.take().is_some()
    }

    /// None where the heap refused, so the next asks again.
    fn write(&mut self, draw: &mut DrawList, shape: Shape, lane: &[u8]) {
        self.shape = self.held.write(draw, lane).then_some(shape);
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        let Some(shape) = self.shape else { return };
        match shape {
            Shape::Box { size_bits, .. } => {
                let t = DrawList::translation(on_whole_units(self.at).0, eye);
                // Written in a cell of its own size (write_lone_box).
                let unit = f64::from_bits(size_bits) as f32;
                draw.draw_in(PIPELINE_GHOST_BOX, self.held.handle, 1, 0, t, GHOST_OPACITY, unit, self.z_step as f32);
            }
            Shape::Decor { group, .. } => draw.draw_instances(PIPELINE_GHOST_DECOR, self.held.handle, group, 1, [0, 1, 6], DrawList::translation(self.at, eye), GHOST_OPACITY),
        }
    }
}
