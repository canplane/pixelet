// pxl-core/domain/scene/marker/set.rs

//! The markers, drawn flat to the camera as one set of instances, each at the
//! place it stands.

use pxl_render::frame::shader::record::PIPELINE_MARKER;
use pxl_render::gpu::api::bytes;
use pxl_render::gpu::draw::{DrawId, DrawList, MirroredBlock, Position};
use pxl_spec::space::coord::q3::Q3;

use crate::domain::scene::marker::atlas::atlas;
use pxl_render::frame::shader::word;

/// The words of one marker instance: its stand as Q3 (3), then the bits of
/// these floats: its logical size in pixels (2), its atlas rectangle (4),
/// where the anchor lies down the sprite, from 0 at the top to 1 at the
/// bottom (1), and its region's width (1). marker.wgsl reads this stride
/// (pxl-render frame/shader/word.rs `MARKER_INSTANCE_WORDS`).
pub(super) const INSTANCE_WORDS: usize = word::MARKER_INSTANCE_WORDS as usize;

/// One instance, anchored at `stands`, `size` logical pixels big, drawn from
/// the atlas's `slot`, with its anchor `down` the sprite. `region` is the
/// width of the region it keeps, as tall as the marker and centered on it. A
/// marker is hidden whole when a nearer marker's region lies in front of its
/// anchor (marker.wgsl). Its stand is a Q3 position, which the GPU takes
/// against the eye's, as it does a draw's (pxl-render gpu/draw.rs
/// `Position`), so nothing here reads the eye.
pub(super) fn instance(
    stands: Q3,
    size: [f64; 2],
    slot: u32,
    down: f64,
    region: f64,
) -> [u32; INSTANCE_WORDS] {
    let atlas = atlas();
    let floats = [
        size[0],
        size[1],
        atlas.u0(slot),
        atlas.v0(slot),
        atlas.u1(slot),
        atlas.v1(slot),
        down,
        region.max(size[0]),
    ]
    .map(|v| (v as f32).to_bits());
    let mut words = [0; INSTANCE_WORDS];
    words[..3].copy_from_slice(&stands.map(|v| v as u32));
    words[3..].copy_from_slice(&floats);
    words
}

/// The markers the arrangement laid out (`arrangement.rs`), in one heap block
/// the renderer mirrors.
#[derive(Default)]
pub(super) struct Set {
    block: MirroredBlock,
    cnt: u32,
    id: DrawId,
}

impl Set {
    /// `instances` holds `INSTANCE_WORDS` words for each instance
    /// (`instance`). When the block cannot be written, nothing is drawn.
    pub(super) fn write(&mut self, draw: &mut DrawList, instances: &[u32]) {
        let written = self.block.write(draw, bytes(instances));
        self.cnt = if written {
            (instances.len() / INSTANCE_WORDS) as u32
        } else {
            0
        };
    }

    /// Frees the block. Returns whether there was one.
    pub(super) fn free(&mut self, draw: &mut DrawList) -> bool {
        let had_block = self.block.handle != 0;
        self.block.free(draw);
        self.cnt = 0;
        had_block
    }

    pub(super) fn draw(&self, draw: &mut DrawList) {
        if self.cnt > 0 && self.block.handle != 0 {
            draw.draw_as(
                &self.id,
                PIPELINE_MARKER,
                self.block.handle,
                self.cnt,
                0,
                Position::NONE,
                None,
            );
        }
    }
}

#[cfg(test)]
#[path = "set.test.rs"]
mod test;
