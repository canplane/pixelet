// pxl-kernel/layer/decor/kind/train.rs

//! A decor kind — occasional trains on rail lines — from kind.json, where its seed and rules are
//! said and why, and the kind as a bundle: those rules bound to the models
//! kind.json lists for it.

use crate::global::Global;
use crate::layer::decor::build::{bind_models, DecorKindBundle};
use crate::layer::decor::kind::spec::decor_kinds::train as S;
use crate::layer::decor::kind::spec::DecorKind;
use crate::layer::decor::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorRules};

pub const TRAIN_KIND: DecorKindSeed = DecorKindSeed {
    tag: S::tag,
    density: S::density,
    variant_count: S::models.len() as u32,
    orientation: S::orientation,
};

#[allow(non_snake_case)]
pub fn TRAIN_RULES() -> DecorRules {
    rules_of(&[], &[], &[], &[], &S::rules::transport.map(|t| t as u32))
}

pub fn is_train_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&TRAIN_RULES(), land, water, structure, transport)
}

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TRAIN_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::train, TRAIN_KIND, TRAIN_RULES(), &S::models, &S::skins, None))
}
