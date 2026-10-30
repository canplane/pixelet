// pxl-kernel/domain/scene/model.rs

//! glTF models, each registered once as an uploaded mesh with its joints and
//! clips: the scene's actors play them and decor stands them over the ground. The
//! draw list sits here, beside its lowest reader: a model's mesh is uploaded
//! through it, and the scene lists into it.

use pxl_render::frame::draw::DrawList;
use xpute_guest::global::Global;

use crate::domain::scene::model::registry::Models;

pub mod asset;
pub mod registry;

/// The registered models, one registry, the models' own.
static MODELS: Global<Models> = Global::new();

pub fn models() -> &'static mut Models {
    MODELS.get(Models::default)
}

/// What the renderer is handed: every layer lists into it.
static DRAW_LIST: Global<DrawList> = Global::new();

pub fn draw_list() -> &'static mut DrawList {
    DRAW_LIST.get(DrawList::default)
}
