// pxl-kernel/domain/query/decor/asset.rs

//! Every decor's models, registered as any model is (`model_of`, the one
//! table, which a landing wakes a turn for), as the renderer reads them. A
//! skin is the load cache's (`load_image`), asked of it where read.

use crate::domain::query::model::asset::{model_of, registered};
use crate::sys::io::loader::{load_image, load_of};
use crate::sys::sched::edge::request_world_tick;
use pxl_spec::asset::decor::{Decor, DECORS};

/// One a variant at most, each its draw group (pxl-spec-core scatter.rs
/// `GROUPS`): two variants naming one model reach one (`model_of`).
pub const MODELS_MAX: usize = pxl_spec_core::scatter::GROUPS as usize;

/// Asks for each variant's model, a turn's effect: registers the first one
/// that has landed and is not registered yet, and asks another turn for the
/// rest, so arrivals are registered a variant a turn.
pub fn decor_models_step() {
    let mut variants = DECORS.iter().flat_map(|decor| decor.variants.iter());
    if variants.any(|v| registered(v.model, None).is_none() && model_of(v.model, None).is_some()) {
        request_world_tick();
    }
}

/// `decor`'s `variant`'s model as registered; none while it loads.
pub fn decor_model(decor: &Decor, variant: usize) -> Option<u32> {
    registered(decor.variants.get(variant)?.model, None).map(|h| h.model)
}

/// A variant's skin as the renderer takes it (`load_image`); none while it
/// loads.
pub fn decor_skin(decor: &Decor, variant: usize) -> Option<(u32, &'static [u8])> {
    load_of(decor.variants.get(variant)?.skin).and_then(load_image)
}
