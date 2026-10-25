// pxl-kernel/domain/scene/marker/sheet.rs

//! The one sheet every marker draws into, mirrored by the renderer.

use pxl_render::gpu::sheet::shelf::Sheet;
use xpute_guest::global::Global;

static SHEET: Global<Sheet> = Global::new();

pub fn sheet() -> &'static mut Sheet {
    SHEET.get(Sheet::new)
}
