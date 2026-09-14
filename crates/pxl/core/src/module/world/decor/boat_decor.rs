// pxl_core/module/world/decor/boat_decor.rs

//! The boat kind as a bundle: boat_kind.rs's placement rules bound to the
//! palette's models for it, which is where the models, their skins and their
//! extents come from (pxl_shared::record::shipped).

use super::decor_kinds::{bind_palette, DecorKindBundle};
use super::boat_kind::{BOAT_KIND, BOAT_RULES};
use crate::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn BOAT_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_palette("boat", BOAT_KIND(), BOAT_RULES(), None))
}
