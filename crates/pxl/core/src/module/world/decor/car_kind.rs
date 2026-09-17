// pxl_core/module/world/decor/car_kind.rs

//! A decor kind — ambient cars on major roads — from kind.json, where its seed and rules are said
//! and why. Deliberately asset-free: the models are bound in car_decor.rs.

use super::kind::decor_kinds::car as S;
use super::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorRules};

#[allow(non_snake_case)]
pub fn CAR_KIND() -> DecorKindSeed {
    DecorKindSeed {
        tag: S::tag,
        density: S::density,
        variant_count: S::models.len() as u32,
        orientation: S::orientation,
    }
}

#[allow(non_snake_case)]
pub fn CAR_RULES() -> DecorRules {
    rules_of(&[], &[], &[], &[], &S::rules::transport.map(|t| t as u32))
}

pub fn is_car_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&CAR_RULES(), land, water, structure, transport)
}
