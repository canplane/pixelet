// pxl_core/module/world/decor/train_decor.rs

//! The train kind as a bundle: train_kind.rs's placement rules bound to the
//! palette's models for it, which is where the models, their skins and their
//! extents come from (pxl_shared::record::shipped).

use super::decor_kinds::{bind_palette, DecorKindBundle};
use super::train_kind::{TRAIN_KIND, TRAIN_RULES};
use crate::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TRAIN_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_palette("train", TRAIN_KIND(), TRAIN_RULES(), None))
}
