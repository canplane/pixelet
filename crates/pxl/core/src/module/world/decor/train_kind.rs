// pxl_core/module/world/decor/train_kind.rs

//! A decor kind — occasional trains on rail lines — from kind.json, where its seed and rules are said
//! and why. Deliberately asset-free: the models are bound in train_decor.rs.

use super::kind::decor_kinds::train as S;
use super::scatter::{eligibility_of, rule_of, rule_word, DecorKindSeed, DecorOrientationMode, DecorRules};

#[allow(non_snake_case)]
pub fn TRAIN_KIND() -> DecorKindSeed {
    DecorKindSeed {
        tag: S::tag,
        density: S::density,
        variant_count: S::models.len() as u32,
        orientation: DecorOrientationMode::of(S::orientation),
    }
}

#[allow(non_snake_case)]
pub fn TRAIN_RULES() -> DecorRules {
    DecorRules {
        land: rule_word(S::rules::land),
        water: rule_word(S::rules::water),
        structure: rule_word(S::rules::structure),
        transport: rule_of("transport", S::rules::transport),
    }
}

pub fn is_train_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&TRAIN_RULES(), land, water, structure, transport)
}
