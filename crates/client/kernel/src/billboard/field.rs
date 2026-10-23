// pxl-kernel/billboard/field.rs

//! Billboards: labels and bubbles drawn flat to the camera, as sets of
//! instances, each set about an origin.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_BADGE};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::v3::V3;

use crate::billboard::sheet::billboard_sheet;

/// Anchor relative to the set's (3), logical size in pixels (2), sheet
/// rectangle (4), anchor's place down the sprite, 0 top 1 bottom (1), and
/// its region's width (1). badge.wgsl reads the same stride.
pub const BILLBOARD_FLOATS: usize = 11;

/// One instance: its anchor `at`, `size` in logical pixels, the sheet's
/// `slot`, where down the sprite the anchor stands, and the width of the
/// region it keeps, as tall as it and centered on it: a billboard is one
/// thing, hidden whole where a nearer one's region is in front of its anchor
/// (badge.wgsl).
pub fn billboard_instance(at: V3, size: [f64; 2], slot: u32, down: f64, region: f64) -> [f32; BILLBOARD_FLOATS] {
    let sheet = billboard_sheet();
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

/// The billboards laid out, about an origin, in one heap block the renderer
/// mirrors: the place step's (`scene/place.rs`).
#[derive(Default)]
pub struct BillboardSet {
    origin: V3,
    held: Held,
    count: u32,
}

impl BillboardSet {
    /// `lane` holds BILLBOARD_FLOATS floats an instance, relative to `origin`.
    pub fn write(&mut self, draw: &mut DrawList, origin: V3, lane: &[f32]) {
        let written = self.held.write(draw, bytes(lane));
        self.count = if written { (lane.len() / BILLBOARD_FLOATS) as u32 } else { 0 };
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
            draw.draw_as(PIPELINE_BADGE, self.held.handle, self.count, 0, DrawList::translation(self.origin, eye), 0.0, 1.0);
        }
    }
}
