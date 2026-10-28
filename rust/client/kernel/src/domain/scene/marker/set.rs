// pxl-kernel/domain/scene/marker/set.rs

//! Markers drawn flat to the camera, as sets of instances, each set about
//! an origin.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_MARKER};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::v3::V3;

use crate::domain::scene::marker::sheet::sheet;

/// Anchor relative to the set's (3), logical size in pixels (2), sheet
/// rectangle (4), anchor's place down the sprite, 0 top 1 bottom (1), and
/// its region's width (1): the stride marker.wgsl reads (pxl-render
/// frame/shader/word.rs `MARKER_INSTANCE_FLOATS`).
pub const INSTANCE_FLOATS: usize = pxl_render::frame::shader::word::MARKER_INSTANCE_FLOATS as usize;

/// One instance: its anchor `at`, `size` in logical pixels, the sheet's
/// `slot`, where down the sprite the anchor stands, and the width of the
/// region it keeps, as tall as it and centered on it: a marker is one
/// thing, hidden whole where a nearer one's region is in front of its anchor
/// (marker.wgsl).
pub fn instance(
    at: V3,
    size: [f64; 2],
    slot: u32,
    down: f64,
    region: f64,
) -> [f32; INSTANCE_FLOATS] {
    let sheet = sheet();
    [
        at[0],
        at[1],
        at[2],
        size[0],
        size[1],
        sheet.u0(slot),
        sheet.v0(slot),
        sheet.u1(slot),
        sheet.v1(slot),
        down,
        region.max(size[0]),
    ]
    .map(|v| v as f32)
}

/// The markers laid out, about an origin, in one heap block the renderer
/// mirrors: the arrangement's (`arrangement.rs`).
#[derive(Default)]
pub struct Set {
    origin: V3,
    held: Held,
    count: u32,
}

impl Set {
    /// `lane` holds INSTANCE_FLOATS floats an instance, relative to `origin`.
    pub fn write(&mut self, draw: &mut DrawList, origin: V3, lane: &[f32]) {
        let written = self.held.write(draw, bytes(lane));
        self.count = if written {
            (lane.len() / INSTANCE_FLOATS) as u32
        } else {
            0
        };
        self.origin = origin;
    }

    /// Whether it held a block.
    pub fn free(&mut self, draw: &mut DrawList) -> bool {
        let held = self.held.bytes > 0;
        self.held.free(draw);
        self.count = 0;
        held
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        if self.count > 0 && self.held.handle != 0 {
            draw.draw_as(
                PIPELINE_MARKER,
                self.held.handle,
                self.count,
                0,
                DrawList::translation(self.origin, eye),
                0.0,
                1.0,
            );
        }
    }
}
