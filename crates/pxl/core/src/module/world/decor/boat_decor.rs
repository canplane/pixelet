// pxl_core/module/world/decor/boat_decor.rs

//! The boat kind as a bundle: boat_kind.rs's placement rules bound to the
//! models kind.json lists for it.

use super::boat_kind::{BOAT_KIND, BOAT_RULES};
use super::decor_layer::{bind_models, DecorKindBundle};
use super::kind::decor_kinds::boat as S;
use super::kind::DecorKind;
use crate::sys::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn BOAT_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::boat, BOAT_KIND(), BOAT_RULES(), &S::models, &S::skins, None))
}
