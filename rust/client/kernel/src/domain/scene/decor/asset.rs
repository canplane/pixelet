// pxl-kernel/domain/scene/decor/asset.rs

//! Every decor's models, registered as any model is (`model_of`, the one
//! table, which a landing wakes a turn for), as the renderer reads them. A
//! skin is the load cache's (`load_image`), asked of it where read.

use crate::domain::scene::model::asset::{model_of, registered_model};
use crate::sys::io::loader::{load_image, load_of};
use pxl_content::generation;
use pxl_spec::asset::decor::{Decor, DECORS};

/// One a variant at most, each its draw group (pxl-content scatter.rs
/// `GROUPS`): two variants naming one model reach one (`model_of`).
pub const MODELS_MAX: usize = generation::GROUPS as usize;

/// Asks for each variant's model, a turn's effect: each registers in its
/// own task, sliced by the turn's deadline (`model_of`).
pub fn decor_models_step() {
    for v in DECORS.iter().flat_map(|decor| decor.variants.iter()) {
        model_of(v.model, None);
    }
}

/// `decor`'s `variant`'s model as registered; none while it loads.
pub fn decor_model(decor: &Decor, variant: usize) -> Option<u32> {
    registered_model(decor.variants.get(variant)?.model, None)
}

/// A variant's skin as the renderer takes it (`load_image`); none while it
/// loads.
pub fn decor_skin(decor: &Decor, variant: usize) -> Option<(u32, &'static [u8])> {
    load_of(decor.variants.get(variant)?.skin).and_then(load_image)
}
