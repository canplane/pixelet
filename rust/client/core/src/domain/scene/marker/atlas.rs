// pxl-core/domain/scene/marker/atlas.rs

//! The one atlas every marker draws into, mirrored by the renderer.

use pxl_render::frame::shelf::Atlas;
use xpute_core::global::Global;

static ATLAS: Global<Atlas> = Global::new();

pub fn atlas() -> &'static mut Atlas {
    ATLAS.get(Atlas::new)
}
