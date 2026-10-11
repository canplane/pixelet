// pxl-kernel/billboard/sheet.rs

//! The one sheet every billboard draws into, mirrored by the renderer, so
//! every set shares one pipeline and binding; and the sets themselves.

use crate::laid;
use pxl_render::gpu::sheet::shelf::Sheet;
use pxl_spec::space::coord::v3::V3;

use crate::kernel;
use xpute_guest::global::Global;

static SHEET: Global<Sheet> = Global::new();

pub fn billboard_sheet() -> &'static mut Sheet {
    SHEET.get(Sheet::new)
}

pub fn new_billboard_set() -> u32 {
    kernel().billboards.new_set()
}

pub fn place_billboard_set(set: u32, origin: V3) {
    kernel().billboards.place(set, origin);
}

pub fn write_billboard_set(set: u32, lane: &[f32]) {
    let Some(k) = laid() else { return };
    k.billboards.write(&mut k.draw, set, lane);
}

pub fn remove_billboard_set(set: u32) {
    let k = kernel();
    k.billboards.remove(&mut k.draw, set);
}

/// One set: the place step lays out every billboard (`chunk/place.rs`).
pub const SETS_MAX: usize = 1;
