// pxl-core/domain/scene/marker/atlas.rs

//! The one atlas every marker draws into, mirrored by the renderer.

use pxl_global::Global;
use pxl_render::frame::shelf::Atlas;

static ATLAS: Global<Atlas> = Global::new();

pub(crate) fn atlas() -> &'static mut Atlas {
    ATLAS.get(Atlas::new)
}
