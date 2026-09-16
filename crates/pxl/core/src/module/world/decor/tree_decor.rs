// pxl_core/module/world/decor/tree_decor.rs

//! The tree kind as a bundle: tree_kind.rs's placement rules bound to the
//! palette's models for it, which is where the models, their skins and their
//! extents come from (pxl_shared::record::shipped).

use super::decor_kinds::{bind_palette, DecorKindBundle};
use super::tree_kind::{TREE_KIND, TREE_RULES};
use crate::sys::global::Global;

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TREE_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_palette("tree", TREE_KIND(), TREE_RULES(), Some("tree")))
}
