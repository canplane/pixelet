//! The hover ghost: what a placement or an erase would change, drawn half
//! transparent where the cursor is. module/world/marker/hover_cursor.tsx
//! decides which ghost and where; what it is drawn from is the core's — a
//! box written in the mesher's layout (mesher.rs's write_lone_box), so a
//! ghost cube is lit, textured and edge-shaded by the chunk shader exactly
//! as the cube it previews, or one decor instance in a block of decor.rs's
//! layout, drawn by decor's shader from the same model a placed one uses.

use crate::bake::mesher::{mesh_words, write_lone_box, LONE_BOX_INDICES, LONE_BOX_VERTICES};
use crate::bake::voxel::NO_LAYER;
use crate::render::draw::{DrawList, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR};
use crate::world::build::World;
use crate::world::decor::NO_ENTRY;

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
}

#[derive(Default)]
pub struct Ghost {
    held: Option<Held>,
    /// Where it stands, in units: a box's min corner, a decor instance's base.
    at: [f64; 3],
}

impl Ghost {
    /// A box of `size` at `at`, painted as object value `state` paints, or
    /// as `rgb` with no texture when `state` paints nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn set_box(&mut self, world: &mut World, draw: &mut DrawList, at: [f64; 3], size: f64, state: u32, rgb: u32, coplanar: bool) {
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
        let bytes = mesh_words(LONE_BOX_VERTICES, LONE_BOX_INDICES) * 4;
        let Some(block) = self.replace(world, draw, shape, bytes) else { return };
        // SAFETY: a block of the words asked for, this ghost's until replaced.
        let m = unsafe { core::slice::from_raw_parts_mut(world.volumes.mem().add(block) as *mut u32, bytes as usize / 4) };
        write_lone_box(m, size, rgb, layer);
        self.upload(draw, LONE_BOX_VERTICES, LONE_BOX_INDICES);
    }

    /// One instance of decor group `group`, turned `rotation` about Z, at `at`.
    pub fn set_decor(&mut self, world: &mut World, draw: &mut DrawList, at: [f64; 3], group: u32, rotation: f32) {
        let shape = Shape::Decor {
            group,
            rotation_bits: rotation.to_bits(),
        };
        self.at = at;
        if self.held.as_ref().is_some_and(|h| h.shape == shape) {
            return;
        }
        // A group of one: five float lanes, the flags in a word, band, pos.
        let bytes = 8 * 4;
        let Some(block) = self.replace(world, draw, shape, bytes) else { return };
        // SAFETY: as for the box.
        let m = unsafe { core::slice::from_raw_parts_mut(world.volumes.mem().add(block) as *mut u32, 8) };
        m.copy_from_slice(&[0f32.to_bits(), 0f32.to_bits(), 0f32.to_bits(), rotation.to_bits(), 1f32.to_bits(), 0, NO_ENTRY, 0]);
        self.upload(draw, 0, 0);
    }

    pub fn clear(&mut self, world: &mut World, draw: &mut DrawList) {
        if let Some(h) = self.held.take() {
            draw.free(h.handle);
            world.pools[world.volumes.mesh_pool()].release(h.at, h.bytes);
        }
    }

    fn replace(&mut self, world: &mut World, draw: &mut DrawList, shape: Shape, bytes: u32) -> Option<usize> {
        self.clear(world, draw);
        let at = world.pools[world.volumes.mesh_pool()].acquire(bytes)?;
        self.held = Some(Held { shape, at, bytes, handle: 0 });
        Some(at)
    }

    fn upload(&mut self, draw: &mut DrawList, vertex_cnt: u32, index_cnt: u32) {
        let h = self.held.as_mut().unwrap();
        h.handle = draw.upload(h.at as u32, h.bytes, vertex_cnt, index_cnt);
    }

    /// The ghost's draw this frame, placed against the frame's origin.
    pub fn draw(&self, draw: &mut DrawList, origin: [f64; 3]) {
        let Some(h) = &self.held else { return };
        let t = DrawList::translation(self.at, origin);
        match h.shape {
            Shape::Box { coplanar, .. } => {
                let pipeline = if coplanar { PIPELINE_GHOST_BOX_COPLANAR } else { PIPELINE_GHOST_BOX };
                draw.draw_as(pipeline, h.handle, LONE_BOX_VERTICES, LONE_BOX_INDICES, t, 1.0, GHOST_OPACITY);
            }
            Shape::Decor { group, .. } => draw.draw_as(PIPELINE_GHOST_DECOR, h.handle, group, 1, t, 0.0, GHOST_OPACITY),
        }
    }
}
