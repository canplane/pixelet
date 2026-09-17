// pxl_core/module/world/decor/tree_kind.rs

//! A decor kind — the first concrete kind: trees on green/forest terrain — from kind.json, where its seed and rules are said
//! and why. Deliberately asset-free: the models are bound in tree_decor.rs.

use super::kind::decor_kinds::tree as S;
use super::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorRules};

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
