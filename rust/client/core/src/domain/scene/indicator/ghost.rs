// pxl-core/domain/scene/indicator/ghost.rs

//! The ghost, half transparent: a box the box shader unfolds
//! (pxl-render `mesh::ghost`), or one decor instance drawn by decor's. What it
//! shows is the tool's (`domain/control/edit/ghost.rs`), handed over as
//! an instance in the pointer's frame (`Ghost`); how each kind of it is drawn
//! is decided here: a box's color and texture, and the box that stands in for
//! a decor until it can be drawn as itself.

use pxl_render::frame::shader::record::BOX_WORDS;
use pxl_render::frame::shader::record::{PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR};
use pxl_render::frame::shader::word::NO_LAYER;
use pxl_render::gpu::api::bytes;
use pxl_render::gpu::draw::{DrawId, DrawList, MirroredBlock, Position, Stand};
use pxl_render::look::color::indicator;
use pxl_render::mesh::ghost::{on_whole_units, write_lone_box};
use pxl_render::mesh::paint::object::PAINTER;
use pxl_spec::asset::cube::cube_rgb;
use pxl_spec::asset::decor::Decor;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::units;

use crate::domain::preview::{Ghost, GhostBox};
use crate::domain::scene::column::GroupColumns;
use crate::domain::scene::decor::asset::{decor_model, decor_skin};
use crate::domain::scene::transform::Transform;
use crate::domain::view::setting::store::settings_store;
use xpute_core::global::Global;

pub const GHOST_OPACITY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Box {
        size_bits: u64,
        off_bits: [u64; 3],
        rgb: [u8; 3],
        layer: u8,
    },
    Decor {
        group: u32,
        rot_bits: u32,
        scale_bits: u32,
    },
}

#[derive(Default)]
pub struct GhostDraw {
    /// What `block` holds; none where nothing is drawn.
    shape: Option<Shape>,
    block: MirroredBlock,
    /// A box's, as written (write_lone_box).
    z_step: f64,
    /// A box's min corner, a decor instance's base.
    at: V3,
    /// Its draw, a box's or a decor's.
    id: DrawId,
}

static GHOST: Global<GhostDraw> = Global::new();

/// The ghost as drawn.
fn shown() -> &'static mut GhostDraw {
    GHOST.get(GhostDraw::default)
}

/// What the tool shows (`GhostDraw::set`); whether what is drawn changed.
pub fn ghost_set(draw: &mut DrawList, ghost: &Ghost) -> bool {
    shown().set(draw, ghost)
}

/// The ghost into the frame's list (`GhostDraw::draw`).
pub fn ghost_draw(draw: &mut DrawList) {
    shown().draw(draw);
}

impl GhostDraw {
    /// What the tool shows, drawn as its kind is. Whether what is drawn
    /// changed.
    pub fn set(&mut self, draw: &mut DrawList, ghost: &Ghost) -> bool {
        match *ghost {
            Ghost::None => self.clear(draw),
            Ghost::Decor {
                at,
                decor,
                variant,
                rot,
                scale,
                stand_in,
            } => {
                let t = Transform {
                    at,
                    rot: rot as f32,
                    scale: scale as f32,
                };
                self.set_decor(draw, decor, variant, t, stand_in)
            }
            Ghost::Box(b) => self.set_box(draw, b),
        }
    }

    /// A box at its corner, in the cube of its zoom, painted as its cube paints
    /// where textures are on, else untextured in its color, or where there
    /// is no cube in the indicator's (`ERASE` for the eraser's, else
    /// `ACTION`). Whether what is drawn changed.
    fn set_box(
        &mut self,
        draw: &mut DrawList,
        GhostBox {
            at,
            zoom,
            cube,
            erase,
        }: GhostBox,
    ) -> bool {
        let size = units(zoom);
        let state = cube.filter(|_| settings_store().textures).unwrap_or(0);
        let rgb = cube.and_then(cube_rgb).unwrap_or(if erase {
            indicator::ERASE
        } else {
            indicator::ACTION
        });
        let (rgb, layer) = PAINTER.look_of(state).unwrap_or((
            [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8],
            NO_LAYER as u8,
        ));
        let off = on_whole_units(at).1;
        let shape = Shape::Box {
            size_bits: size.to_bits(),
            off_bits: off.map(f64::to_bits),
            rgb,
            layer,
        };
        let moved = core::mem::replace(&mut self.at, at) != at;
        if self.shape == Some(shape) {
            return moved;
        }
        let mut m = [0; BOX_WORDS as usize];
        self.z_step = write_lone_box(&mut m, off, size, rgb, layer);
        self.write(draw, shape, bytes(&m));
        true
    }

    /// `decor`'s `variant`, its base at `t`'s `at`. Until the variant's model
    /// and skin both arrive `stand_in`'s box is drawn instead; with none, the
    /// instance is the world's own and drawn as it is. Whether what is drawn
    /// changed.
    fn set_decor(
        &mut self,
        draw: &mut DrawList,
        decor: &Decor,
        variant: u32,
        t: Transform,
        stand_in: Option<GhostBox>,
    ) -> bool {
        let Transform { at, rot, scale } = t;
        let ready = || {
            decor_model(decor, variant as usize).is_some()
                && decor_skin(decor, variant as usize).is_some()
        };
        if let Some(b) = stand_in.filter(|_| !ready()) {
            return self.set_box(draw, b);
        }
        let group = decor.first_group() + variant;
        let shape = Shape::Decor {
            group,
            rot_bits: rot.to_bits(),
            scale_bits: scale.to_bits(),
        };
        let moved = core::mem::replace(&mut self.at, at) != at;
        if self.shape == Some(shape) {
            return moved;
        }
        // One group of one, with no header: its columns, its flags, and the
        // index list.
        let column = GroupColumns { first: 0, cnt: 1 };
        let mut m = [0u32; GroupColumns::words(1) + 1];
        (m[column.f32_at(3, 0)], m[column.f32_at(4, 0)]) = (rot.to_bits(), scale.to_bits());
        self.write(draw, shape, bytes(&m));
        true
    }

    /// Whether there was one.
    pub fn clear(&mut self, draw: &mut DrawList) -> bool {
        self.block.free(draw);
        self.shape.take().is_some()
    }

    /// None where the heap refused, so the next asks again.
    fn write(&mut self, draw: &mut DrawList, shape: Shape, instance: &[u8]) {
        self.shape = self.block.write(draw, instance).then_some(shape);
    }

    /// On Q3, a box's on whole units, a decor's where its stored Q3 is: on any lap of X,
    /// drawn the short way round from the eye (pxl-render gpu/draw.rs `Position`).
    pub fn draw(&self, draw: &mut DrawList) {
        let Some(shape) = self.shape else { return };
        match shape {
            Shape::Box { size_bits, .. } => {
                let whole = on_whole_units(self.at).0;
                // Written in a cell of its own size (write_lone_box).
                let size = f64::from_bits(size_bits);
                // It reads the sun's map, so it stands within its cell, its corner off the whole
                // unit by under one.
                let stand = Stand::of(whole, whole.map(|v| v + 1.0 + size));
                draw.draw_in(
                    &self.id,
                    PIPELINE_GHOST_BOX,
                    self.block.handle,
                    1,
                    0,
                    Position::of(whole),
                    GHOST_OPACITY,
                    size as f32,
                    self.z_step as f32,
                    Some(stand),
                );
            }
            Shape::Decor { group, .. } => {
                let idx = GroupColumns::words(1) as u32;
                draw.draw_instances(
                    &self.id,
                    PIPELINE_GHOST_DECOR,
                    self.block.handle,
                    group,
                    1,
                    [0, 1, idx],
                    Position::of(self.at),
                    GHOST_OPACITY,
                    false,
                    Some(Stand::ball(self.at)),
                );
            }
        }
    }
}
