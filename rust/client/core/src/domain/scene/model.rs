// pxl-core/domain/scene/model.rs

//! glTF models. Each model is registered once, as an uploaded mesh with its
//! joints and clips. The scene's characters play a model's clips, and decor
//! places models on the ground. The draw list lives here because a model's
//! mesh is uploaded through it, and this is the lowest module that needs it.

use pxl_global::Global;
use pxl_render::gpu::draw::DrawList;

use crate::domain::scene::model::registry::ModelTable;

pub(crate) mod asset;
pub(crate) mod registry;

static MODELS: Global<ModelTable> = Global::new();

pub(crate) fn models() -> &'static mut ModelTable {
    MODELS.get(ModelTable::default)
}

/// The list the renderer is handed each frame. Every drawn layer adds its draws
/// to it.
static DRAW_LIST: Global<DrawList> = Global::new();

pub(crate) fn draw_list() -> &'static mut DrawList {
    DRAW_LIST.get(DrawList::default)
}
