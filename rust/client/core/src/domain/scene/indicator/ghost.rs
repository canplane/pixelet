// pxl-core/domain/scene/indicator/ghost.rs

//! The hover ghost, drawn half transparent: either a box, which the box
//! shader unfolds from its draw's uniform (pxl-render box.wgsl), or one decor
//! instance, which decor's shader draws. The tool decides what the ghost shows
//! (domain/control/edit/ghost.rs) and hands it over as an instance in the
//! pointer's frame (`Ghost`). This module decides how each kind is drawn: a
//! box's color and texture, and the box that stands in for a decor until the
//! decor can be drawn as itself.

use pxl_render::frame::shader::record::PIPELINE_GHOST_DECOR;
use pxl_render::frame::shader::word::NO_LAYER;
use pxl_render::gpu::api::bytes;
use pxl_render::gpu::draw::{DrawId, DrawList, MirroredBlock, Position, ReceiverBox};
use pxl_render::look::color::indicator;
use pxl_render::mesh::paint::object::PAINTER;
use pxl_spec::asset::cube::cube_rgb;
use pxl_spec::asset::decor::Decor;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::units;

use crate::domain::preview::{Ghost, GhostBox};
use crate::domain::scene::column::GroupRange;
use crate::domain::scene::decor::asset::{decor_model, decor_skin};
use crate::domain::scene::indicator::param::GHOST_OPACITY;
use crate::domain::scene::transform::Transform;
use crate::domain::setting::store::settings;
use xpute_core::global::Global;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Box {
        side_bits: u64,
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
    /// What is drawn, or None when nothing is.
    shape: Option<Shape>,
    /// A decor's one instance; a box has no block.
    block: MirroredBlock,
    /// A box's min corner, or a decor instance's base.
    at: V3,
    id: DrawId,
}

static GHOST: Global<GhostDraw> = Global::new();

fn shown() -> &'static mut GhostDraw {
    GHOST.get(GhostDraw::default)
}

/// Draws what the tool shows (`GhostDraw::set`). Returns whether what is
/// drawn changed.
pub fn ghost_set(draw: &mut DrawList, ghost: &Ghost) -> bool {
    shown().set(draw, ghost)
}

pub fn ghost_draw(draw: &mut DrawList) {
    shown().draw(draw);
}

impl GhostDraw {
    /// Sets the ghost to what the tool shows, drawn as its kind is drawn.
    /// Returns whether what is drawn changed.
    pub fn set(&mut self, draw: &mut DrawList, ghost: &Ghost) -> bool {
        match *ghost {
            Ghost::None => self.clear(draw),
            Ghost::Decor {
                at,
                decor,
                variant,
                rot,
                stand_in,
            } => {
                let t = Transform {
                    at,
                    rot: rot as f32,
                    scale: 1.0,
                };
                self.set_decor(draw, decor, variant, t, stand_in)
            }
            Ghost::Box(b) => self.set_box(draw, b),
        }
    }

    /// Sets the ghost to a box at its corner, the size of a cube at its zoom.
    /// With textures on, it is painted as its cube paints. With textures off,
    /// it is drawn untextured in its cube's color. With no cube, it takes the
    /// indicator's color: `ERASE` for the eraser and `ACTION` otherwise.
    /// Returns whether what is drawn changed.
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
        let side = units(zoom);
        let state = cube.filter(|_| settings().textures).unwrap_or(0);
        let rgb = cube.and_then(cube_rgb).unwrap_or(if erase {
            indicator::ERASE
        } else {
            indicator::ACTION
        });
        let (rgb, layer) = PAINTER.look_of(state).unwrap_or((
            [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8],
            NO_LAYER as u8,
        ));
        let shape = Shape::Box {
            side_bits: side.to_bits(),
            rgb,
            layer,
        };
        let moved = core::mem::replace(&mut self.at, at) != at;
        if self.shape == Some(shape) {
            return moved;
        }
        self.block.free(draw);
        self.shape = Some(shape);
        true
    }

    /// Sets the ghost to `variant` of `decor`, with its base at `t.at`. Until
    /// the variant's model and skin have both arrived, the box `stand_in` is
    /// drawn instead. Without a stand-in, the instance is one the world
    /// already holds, and it is drawn as it is. Returns whether what is drawn
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
        // The block is one group of one instance, with no header: its
        // columns, its flags, and the index list.
        let column = GroupRange { first: 0, cnt: 1 };
        let mut m = [0u32; GroupRange::words(1) + 1];
        (m[column.f32_at(3, 0)], m[column.f32_at(4, 0)]) = (rot.to_bits(), scale.to_bits());
        self.write(draw, shape, bytes(&m));
        true
    }

    /// Removes the ghost. Returns whether there was one.
    pub fn clear(&mut self, draw: &mut DrawList) -> bool {
        self.block.free(draw);
        self.shape.take().is_some()
    }

    /// When the heap refuses the write, `shape` is left None, so the next set
    /// writes again.
    fn write(&mut self, draw: &mut DrawList, shape: Shape, instance: &[u8]) {
        self.shape = self.block.write(draw, instance).then_some(shape);
    }

    /// Lists the ghost's draw at a Q3 position: a box's min corner or a decor's stored
    /// position. Either may lie on any lap of X, since the renderer draws it the short way
    /// round from the eye (pxl-render gpu/draw.rs `Position`).
    pub fn draw(&self, draw: &mut DrawList) {
        let Some(shape) = self.shape else { return };
        match shape {
            Shape::Box {
                side_bits,
                rgb,
                layer,
            } => {
                let side = f64::from_bits(side_bits);
                // The box reads the sun's map, so it declares the box it stands
                // within: its cell, grown by a unit, since its corner lies less
                // than a unit off the whole unit.
                let whole = self.at.map(f64::floor);
                let stand = ReceiverBox::of(whole, whole.map(|v| v + 1.0 + side));
                draw.draw_box(
                    &self.id,
                    Position::of(self.at),
                    GHOST_OPACITY,
                    side as f32,
                    u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]),
                    u32::from(layer),
                    Some(stand),
                );
            }
            Shape::Decor { group, .. } => {
                let idx = GroupRange::words(1) as u32;
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
                    Some(ReceiverBox::ball(self.at)),
                );
            }
        }
    }
}
