// pxl-kernel/billboard/sheet.rs

//! The one sheet every billboard draws into, mirrored by the renderer.

use pxl_render::gpu::sheet::shelf::Sheet;
use xpute_guest::global::Global;

static SHEET: Global<Sheet> = Global::new();

pub fn billboard_sheet() -> &'static mut Sheet {
    SHEET.get(Sheet::new)
}
