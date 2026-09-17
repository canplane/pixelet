// pxl_core/module/world/decor/train_decor.rs

//! The train kind as a bundle: train_kind.rs's placement rules bound to the
//! models kind.json lists for it.

use super::decor_layer::{bind_models, DecorKindBundle};
use super::kind::decor_kinds::train as S;
use super::kind::DecorKind;
use super::train_kind::{TRAIN_KIND, TRAIN_RULES};
use crate::sys::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TRAIN_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::train, TRAIN_KIND(), TRAIN_RULES(), &S::models, &S::skins, None))
}
