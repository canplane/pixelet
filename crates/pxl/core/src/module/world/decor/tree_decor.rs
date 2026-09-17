// pxl_core/module/world/decor/tree_decor.rs

//! The tree kind as a bundle: tree_kind.rs's placement rules bound to the
//! models kind.json lists for it.

use super::decor_layer::{bind_models, DecorKindBundle};
use super::kind::decor_kinds::tree as S;
use super::kind::DecorKind;
use super::tree_kind::{TREE_KIND, TREE_RULES};
use crate::sys::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TREE_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::tree, TREE_KIND(), TREE_RULES(), &S::models, &S::skins, Some(S::decor_id)))
}
