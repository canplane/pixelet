// pxl-kernel/domain/scene/marker/set.rs

//! Markers drawn flat to the camera, as one set of instances, each where
//! it stands.

use pxl_render::frame::draw::{DrawList, MirroredBlock, Position, PIPELINE_MARKER};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::q3::Q3;

use crate::domain::scene::marker::atlas::atlas;
use pxl_render::frame::shader::word;

/// Its stand's Q3 (3), logical size in pixels (2), atlas rectangle (4),
/// anchor's place down the sprite, 0 top 1 bottom (1), and its region's
/// width (1), the floats' bits: the stride marker.wgsl reads (pxl-render
/// frame/shader/word.rs `MARKER_INSTANCE_WORDS`).
pub(super) const INSTANCE_WORDS: usize = word::MARKER_INSTANCE_WORDS as usize;

/// One instance: where its anchor stands, `stands`, `size` in logical
/// pixels, the atlas's `slot`, where down the sprite the anchor stands, and
/// the width of the region it keeps, as tall as it and centered on it: a
/// marker is one thing, hidden whole where a nearer one's region is in front
/// of its anchor (marker.wgsl). Its stand is on Q3 and is taken against the
/// eye's on the GPU, as a draw's is (pxl-render frame/draw.rs `Position`),
/// so nothing here reads the eye.
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

/// The markers laid out, in one heap block the renderer mirrors: the
/// arrangement's (`arrangement.rs`).
#[derive(Default)]
pub(super) struct Set {
    block: MirroredBlock,
    cnt: u32,
}

impl Set {
    /// `instances` holds INSTANCE_WORDS words an instance (`instance`).
    pub(super) fn write(&mut self, draw: &mut DrawList, instances: &[u32]) {
        let written = self.block.write(draw, bytes(instances));
        self.cnt = if written {
            (instances.len() / INSTANCE_WORDS) as u32
        } else {
            0
        };
    }

    /// Whether it held a block.
    pub(super) fn free(&mut self, draw: &mut DrawList) -> bool {
        let held = self.block.bytes > 0;
        self.block.free(draw);
        self.cnt = 0;
        held
    }

    pub(super) fn draw(&self, draw: &mut DrawList) {
        if self.cnt > 0 && self.block.handle != 0 {
            draw.draw_as(
                PIPELINE_MARKER,
                self.block.handle,
                self.cnt,
                0,
                Position::NONE,
                0.0,
                1.0,
                None,
            );
        }
    }
}

#[cfg(test)]
#[path = "set.test.rs"]
mod test;
