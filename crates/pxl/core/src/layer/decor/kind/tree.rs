// pxl_core/layer/decor/kind/tree.rs

//! A decor kind — the first concrete kind: trees on green/forest terrain — from kind.json, where its seed and rules are
//! said and why, and the kind as a bundle: those rules bound to the models
//! kind.json lists for it.

use crate::layer::decor::build::{bind_models, DecorKindBundle};
use crate::layer::decor::kind::spec::decor_kinds::tree as S;
use crate::layer::decor::kind::spec::DecorKind;
use crate::layer::decor::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorRules};
use crate::sys::global::Global;

#[allow(non_snake_case)]
pub fn TREE_KIND() -> DecorKindSeed {
    DecorKindSeed {
        tag: S::tag,
        density: S::density,
        variant_count: S::models.len() as u32,
        orientation: S::orientation,
    }
}

#[allow(non_snake_case)]
pub fn TREE_RULES() -> DecorRules {
    rules_of(&S::rules::void, &S::rules::land.map(|t| t as u32), &[], &[], &[])
}

pub fn is_tree_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&TREE_RULES(), land, water, structure, transport)
}

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TREE_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::tree, TREE_KIND(), TREE_RULES(), &S::models, &S::skins, Some(S::decor_id)))
}
