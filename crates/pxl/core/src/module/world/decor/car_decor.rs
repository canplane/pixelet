// pxl_core/module/world/decor/car_decor.rs

//! The car kind as a bundle: car_kind.rs's placement rules bound to the
//! palette's models for it, which is where the models, their skins and their
//! extents come from (pxl_shared::record::shipped).

use super::car_kind::{CAR_KIND, CAR_RULES};
use super::decor_layer::{bind_palette, DecorKindBundle};
use crate::sys::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn CAR_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_palette("car", CAR_KIND(), CAR_RULES(), None))
}
