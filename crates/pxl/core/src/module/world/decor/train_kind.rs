// pxl_core/module/world/decor/train_kind.rs

//! A decor kind — occasional trains on rail lines — from kind.json, where its seed and rules are said
//! and why. Deliberately asset-free: the models are bound in train_decor.rs.

use super::kind::decor_kinds::train as S;
use super::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorRules};

#[allow(non_snake_case)]
pub fn TRAIN_KIND() -> DecorKindSeed {
    DecorKindSeed {
        tag: S::tag,
        density: S::density,
        variant_count: S::models.len() as u32,
        orientation: S::orientation,
    }
}

#[allow(non_snake_case)]
pub fn TRAIN_RULES() -> DecorRules {
    rules_of(&[], &[], &[], &[], &S::rules::transport.map(|t| t as u32))
}

pub fn is_train_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&TRAIN_RULES(), land, water, structure, transport)
}
