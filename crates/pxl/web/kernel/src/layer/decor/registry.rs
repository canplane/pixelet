// pxl-kernel/layer/decor/registry.rs

//! Every decor kind's own bundle, in one place. What the scheduler is taught
//! (decor/build.rs's teach_decor_kinds) and the hover ghost's shape resolution
//! (which bundle does a given decor_id's silhouette come from) both need this
//! same lookup — one shared registry instead of two call sites independently
//! assembling "every decor kind" and risking drifting out of sync.

use pxl_spec::cell::DecorId;

use crate::global::Global;
use crate::layer::decor::build::{bind_models, teach_decor_kinds, DecorKindBundle};
use crate::layer::decor::kind::{boat, car, train, tree};

static BUNDLES: Global<[DecorKindBundle; 4]> = Global::new();

/// Every bundle, in group order; the scheduler is taught them the first time
/// they are asked for.
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

/// Which bundle a user-placeable decor_id belongs to — None for a decor_id
/// somehow missing its own bundle, which would itself be a real bug.
pub fn decor_bundle_for(decor_id: DecorId) -> Option<&'static DecorKindBundle> {
    DECOR_BUNDLES().iter().find(|b| b.decor_id == Some(decor_id))
}
