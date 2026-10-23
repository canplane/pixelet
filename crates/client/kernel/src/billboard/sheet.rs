// pxl-kernel/billboard/sheet.rs

//! The one sheet every billboard draws into, mirrored by the renderer, so
//! every set shares one pipeline and binding; and the sets themselves.

use pxl_render::gpu::sheet::shelf::Sheet;
use pxl_spec::space::coord::v3::V3;

use crate::billboard::field::billboards;
use pxl_render::frame::draw::DrawList;
use xpute_guest::global::Global;

static SHEET: Global<Sheet> = Global::new();

pub fn billboard_sheet() -> &'static mut Sheet {
    SHEET.get(Sheet::new)
}

pub fn new_billboard_set() -> u32 {
    billboards().new_set()
}

pub fn place_billboard_set(set: u32, origin: V3) {
    billboards().place(set, origin);
}

/// Into `draw`, the draw list its caller holds.
pub fn write_billboard_set(draw: &mut DrawList, set: u32, lane: &[f32]) {
    billboards().write(draw, set, lane);
}

pub fn remove_billboard_set(draw: &mut DrawList, set: u32) {
    billboards().remove(draw, set);
}

/// One set: the place step lays out every billboard (`scene/place.rs`).
pub const SETS_MAX: usize = 1;
