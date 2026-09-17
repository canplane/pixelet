// pxl_core/module/world/decor/car_decor.rs

//! The car kind as a bundle: car_kind.rs's placement rules bound to the
//! models kind.json lists for it.

use super::car_kind::{CAR_KIND, CAR_RULES};
use super::decor_layer::{bind_models, DecorKindBundle};
use super::kind::decor_kinds::car as S;
use super::kind::DecorKind;
use crate::sys::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn CAR_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::car, CAR_KIND(), CAR_RULES(), &S::models, &S::skins, None))
}
