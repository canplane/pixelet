// pxl_core/layer/decor/kind/boat.rs

//! A decor kind — small boats on open water — from kind.json, where its seed and rules are
//! said and why, and the kind as a bundle: those rules bound to the models
//! kind.json lists for it.

use crate::layer::decor::build::{bind_models, DecorKindBundle};
use crate::layer::decor::kind::spec::decor_kinds::boat as S;
use crate::layer::decor::kind::spec::DecorKind;
use crate::layer::decor::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorRules};
use crate::sys::global::Global;

#[allow(non_snake_case)]
pub fn BOAT_KIND() -> DecorKindSeed {
    DecorKindSeed {
        tag: S::tag,
        density: S::density,
        variant_count: S::models.len() as u32,
        orientation: S::orientation,
    }
}

#[allow(non_snake_case)]
pub fn BOAT_RULES() -> DecorRules {
    rules_of(&S::rules::void, &[], &S::rules::water.map(|t| t as u32), &[], &[])
}

pub fn is_boat_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&BOAT_RULES(), land, water, structure, transport)
}

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn BOAT_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::boat, BOAT_KIND(), BOAT_RULES(), &S::models, &S::skins, None))
}
