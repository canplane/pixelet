// pxl-core/domain/scene/marker/atlas.rs

//! The one atlas every marker draws into, mirrored by the renderer.

use pxl_render::frame::shelf::Atlas;
use pxl_global::Global;

static ATLAS: Global<Atlas> = Global::new();

pub(crate) fn atlas() -> &'static mut Atlas {
    ATLAS.get(Atlas::new)
}
