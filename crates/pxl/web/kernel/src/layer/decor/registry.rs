// pxl-kernel/layer/decor/registry.rs

//! Every decor kind's bundle, in one place, for the scheduler and the hover ghost.

use pxl_spec::cell::DecorId;

use crate::global::Global;
use crate::layer::decor::build::{bind_models, teach_decor_kinds, DecorKindBundle};
use crate::layer::decor::kind::{boat, car, train, tree};

static BUNDLES: Global<[DecorKindBundle; 4]> = Global::new();

/// In group order.
#[allow(non_snake_case)]
pub fn DECOR_BUNDLES() -> &'static [DecorKindBundle; 4] {
    let mut made = false;
    let bundles = BUNDLES.get(|| {
        made = true;
        [tree::DEF, boat::DEF, car::DEF, train::DEF].each_ref().map(bind_models)
    });
    if made {
        teach_decor_kinds(&bundles.each_ref());
    }
    bundles
}

pub fn decor_bundle_for(decor_id: DecorId) -> Option<&'static DecorKindBundle> {
    DECOR_BUNDLES().iter().find(|b| b.decor_id == Some(decor_id))
}
