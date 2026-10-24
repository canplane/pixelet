// pxl-kernel/scene/indicator/ghost.rs

//! The ghost, half transparent: a box the box shader unfolds
//! (pxl-render `mesh::ghost`), or one decor instance drawn by decor's. What it
//! shows is the tool's (`control/interaction/ghost.rs`); how it looks is
//! decided here: a box's color and texture, and the box that stands in for a
//! decor until it can be drawn as itself.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR};
use pxl_render::frame::shader::record::BOX_WORDS;
use pxl_render::frame::shader::words::NO_LAYER;
use pxl_render::gpu::api::bytes;
use pxl_render::look::color::indicator;
use pxl_render::mesh::ghost::{on_whole_units, write_lone_box};
use pxl_render::mesh::paint::object::PAINTER;
use pxl_spec::block::decor::Decor;
use pxl_spec::block::object::{cube_id, cube_rgb, Cube};
use pxl_spec::space::coord::v3::V3;

use crate::decor::assets::decor_assets_for;
use crate::setting::store::settings_store;

pub const GHOST_OPACITY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Box { size_bits: u64, offset_bits: [u64; 3], rgb: [u8; 3], layer: u8 },
    Decor { group: u32, rotation_bits: u32, scale_bits: u32 },
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
    /// A box at `at`, `size` on an edge, painted as `cube` paints where
    /// textures are on, else untextured in its color, or where there is no
    /// cube in the indicator's (`ERASE` for the eraser's, else `ACTION`).
    /// Whether what is drawn changed.
    pub fn set_box(&mut self, draw: &mut DrawList, at: V3, size: f64, cube: Option<Cube>, erase: bool) -> bool {
        let state = cube.filter(|_| settings_store().textures).map_or(0, cube_id);
        let rgb = cube.and_then(cube_rgb).unwrap_or(if erase { indicator::ERASE } else { indicator::ACTION });
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

    /// `decor`'s `variant`, its base at `at`. An untextured white silhouette
    /// reads as a glitch, so until the variant's model and skin both arrive
    /// `stand_in`'s box (`set_box`'s corner, edge, cube and eraser) is drawn
    /// instead; with none, the instance is the world's own and drawn as it
    /// is. Whether what is drawn changed.
    #[allow(clippy::too_many_arguments)]
    pub fn set_decor(&mut self, draw: &mut DrawList, at: V3, decor: &Decor, variant: u32, rotation: f32, scale: f32, stand_in: Option<(V3, f64, Option<Cube>, bool)>) -> bool {
        let ready = || decor_assets_for(decor).is_some_and(|a| a.models.get(variant as usize).is_some_and(Option::is_some) && a.skin(variant as usize).is_some());
        if let Some((at, size, cube, erase)) = stand_in.filter(|_| !ready()) {
            return self.set_box(draw, at, size, cube, erase);
        }
        let group = decor.first_group() + variant;
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
