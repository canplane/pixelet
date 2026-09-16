// pxl_core/module/world/decor/decor_registry.rs

//! Every decor kind's own bundle, in one place. What the scheduler is taught
//! (decor_kinds.rs's teach_decor_kinds) and the hover ghost's shape resolution
//! (which bundle does a given decor_id's silhouette come from) both need this
//! same lookup — one shared registry instead of two call sites independently
//! assembling "every decor kind" and risking drifting out of sync.

use pxl_shared::spec::cell::DecorId;

use super::boat_decor::BOAT_DECOR_BUNDLE;
use super::car_decor::CAR_DECOR_BUNDLE;
use super::decor_kinds::{teach_decor_kinds, DecorKindBundle};
use super::train_decor::TRAIN_DECOR_BUNDLE;
use super::tree_decor::TREE_DECOR_BUNDLE;
use crate::sys::global::Global;

static TAUGHT: Global<bool> = Global::new();

/// Every bundle, in group order; the scheduler is taught them the first time
/// they are asked for.
#[allow(non_snake_case)]
pub fn DECOR_BUNDLES() -> [&'static DecorKindBundle; 4] {
    let bundles = [TREE_DECOR_BUNDLE(), BOAT_DECOR_BUNDLE(), CAR_DECOR_BUNDLE(), TRAIN_DECOR_BUNDLE()];
    let taught = TAUGHT.get(|| false);
    if !*taught {
        *taught = true;
        teach_decor_kinds(&bundles);
    }
    bundles
}

/// Which bundle a user-placeable decor_id belongs to — None for a decor_id
/// somehow missing its own bundle, which would itself be a real bug.
pub fn decor_bundle_for(decor_id: DecorId) -> Option<&'static DecorKindBundle> {
    DECOR_BUNDLES().into_iter().find(|b| b.decor_id == Some(decor_id))
}
