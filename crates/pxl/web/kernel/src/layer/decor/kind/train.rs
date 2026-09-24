// pxl-kernel/layer/decor/kind/train.rs

//! A decor kind: its seed and rules, and the kind as a bundle, those rules
//! bound to its models.
//!
//! Occasional trains on rail lines. rail_main only (not RAIL_SUBWAY —
//! that's underground, wrong context for a surface-scattered prop). Same
//! "align_transport" reasoning as car applies: a train lying across the rail
//! sideways would read as derailed. Only the transport channel matters.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::cell::cube_types::Transport;

use crate::global::Global;
use crate::layer::decor::build::{bind_models, DecorKindBundle, DecorVariantAsset};
use crate::layer::decor::kind::DecorKind;
use crate::layer::decor::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorOrientationMode, DecorRules};

/// The kind's variants, each a model by its path under asset/ and the skin
/// it is drawn with, in the order the scatter picks among them.
const VARIANTS: [DecorVariantAsset; 4] = [
    DecorVariantAsset {
        model: Asset::decor_kenney_train_kit_train_locomotive_a_glb,
        skin: Asset::decor_kenney_train_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_train_kit_train_locomotive_b_glb,
        skin: Asset::decor_kenney_train_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_train_kit_train_diesel_a_glb,
        skin: Asset::decor_kenney_train_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_train_kit_train_electric_city_a_glb,
        skin: Asset::decor_kenney_train_kit_colormap_png,
    },
];

pub const TRAIN_KIND: DecorKindSeed = DecorKindSeed {
    tag: 0x7ba15eed0004dec0,
    // Very sparse, deliberately — trains are meant to read as occasional, not
    // as a train sitting on every visible stretch of track.
    density: 0.004,
    variant_count: VARIANTS.len() as u32,
    orientation: DecorOrientationMode::AlignTransport,
};

#[allow(non_snake_case)]
pub fn TRAIN_RULES() -> DecorRules {
    rules_of(&[], &[], &[], &[], &[Transport::rail_main].map(|t| t as u32))
}

pub fn is_train_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&TRAIN_RULES(), land, water, structure, transport)
}

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TRAIN_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::Train, TRAIN_KIND, TRAIN_RULES(), &VARIANTS, None))
}
