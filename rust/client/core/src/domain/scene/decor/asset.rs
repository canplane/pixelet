// pxl-core/domain/scene/decor/asset.rs

//! The models and skins of every decor variant. Models are registered in the
//! one model table, as every model is (`model_of`), and a model that finishes
//! loading wakes a frame. Skins are not kept here: each reader asks the load
//! cache for one where it reads it (`load_image`).

use crate::domain::scene::model::asset::{model_of, registered_model};
use crate::sys::io::loader::{load_image, load_of};
use pxl_content::generation;
use pxl_spec::asset::decor::{Decor, DECORS};

/// The most models decor registers: one for each variant, and each variant is
/// a draw group (pxl-content generation.rs `GROUPS`). Two variants that name
/// one model share it (`model_of`), so fewer may be registered.
pub const MODELS_MAX: usize = generation::GROUPS as usize;

/// Asks for every variant's model, as one of a frame's effects. Each model
/// registers in a task of its own, whose work is spread over entries by each
/// entry's deadline (`model_of`).
pub fn decor_models_step() {
    for v in DECORS.iter().flat_map(|decor| decor.variants.iter()) {
        model_of(v.model, None);
    }
}

/// The model of `variant` of `decor`, or None while it is not registered yet.
/// It starts no load: `decor_models_step` does.
pub fn decor_model(decor: &Decor, variant: usize) -> Option<u32> {
    registered_model(decor.variants.get(variant)?.model, None)
}

/// The skin of `variant` of `decor` as the renderer takes it (`load_image`),
/// or None while it loads.
pub fn decor_skin(decor: &Decor, variant: usize) -> Option<(u32, &'static [u8])> {
    load_of(decor.variants.get(variant)?.skin).and_then(load_image)
}
