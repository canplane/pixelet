// pxl-kernel/model.rs

//! glTF models, each registered once as an uploaded mesh with its joints and
//! clips: the scene's actors play them and decor stands them in chunks.

use xpute_guest::global::Global;

use crate::model::registry::Models;

pub mod asset;
pub mod registry;

/// The registered models, one registry, the models' own.
static MODELS: Global<Models> = Global::new();

pub fn models() -> &'static mut Models {
    MODELS.get(Models::default)
}
/// The characters a player can be, by their models (variant.json).
#[path = "model/variant.spec.rs"]
pub mod variant_spec;
