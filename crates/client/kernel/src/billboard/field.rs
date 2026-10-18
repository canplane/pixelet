// pxl-kernel/billboard/field.rs

//! Billboards: labels and bubbles drawn flat to the camera, as sets of
//! instances, each set about an origin.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_BADGE};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::abi::handle::Slots;

use crate::billboard::sheet::{billboard_sheet, SETS_MAX};

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

struct Set {
    origin: V3,
    held: Held,
    count: u32,
}

#[derive(Default)]
pub struct Billboards {
    sets: Slots<Set, { SETS_MAX + 1 }>,
}

/// The sets, the billboards' own: a leaf holds what it lays out, as the
/// terrain holds its tiles, and is handed the draw list it writes into.
static BILLBOARDS: xpute_guest::global::Global<Billboards> = xpute_guest::global::Global::new();

pub fn billboards() -> &'static mut Billboards {
    BILLBOARDS.get(Billboards::default)
}

impl Billboards {
    pub fn new_set(&mut self) -> u32 {
        self.sets.insert(Set {
            origin: [0.0; 3],
            held: Held::default(),
            count: 0,
        })
    }

    pub fn write(&mut self, draw: &mut DrawList, id: u32, lane: &[f32]) {
        let Some(s) = self.sets.get_mut(id) else { return };
        let written = s.held.write(draw, bytes(lane));
        s.count = if written { (lane.len() / BILLBOARD_FLOATS) as u32 } else { 0 };
    }

    pub fn place(&mut self, id: u32, origin: V3) {
        if let Some(s) = self.sets.get_mut(id) {
            s.origin = origin;
        }
    }

    pub fn remove(&mut self, draw: &mut DrawList, id: u32) {
        if let Some(mut s) = self.sets.remove(id) {
            s.held.free(draw);
        }
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        for (_, s) in self.sets.iter().filter(|(_, s)| s.count > 0 && s.held.handle != 0) {
            draw.draw_as(PIPELINE_BADGE, s.held.handle, s.count, 0, DrawList::translation(s.origin, eye), 0.0, 1.0);
        }
    }
}
