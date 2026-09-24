// pxl-kernel/layer/decor/kind/car.rs

//! A decor kind: its seed and rules, and the kind as a bundle, those rules
//! bound to its models.
//!
//! Ambient cars on major roads. Motorway/primary/secondary only,
//! deliberately: ordinary neighborhood streets and pedestrian ways
//! (ROAD_DEFAULT, PEDESTRIAN) are dense and everywhere a user might want to
//! build, and a car parked in the middle of one reads as "in the way" far
//! more than the same car sitting on a major road's shoulder does.
//! "align_transport" orientation is what keeps these lined up with the road
//! instead of facing a random direction, which would otherwise read as a
//! crash, not traffic. Only the transport channel matters: transport always
//! wins last in the terrain paint, so a cell whose transport is one of these
//! renders (and therefore counts) as that road type regardless of whatever
//! land/water/structure value sits underneath it.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::cell::cube_types::Transport;

use crate::global::Global;
use crate::layer::decor::build::{bind_models, DecorKindBundle, DecorVariantAsset};
use crate::layer::decor::kind::DecorKind;
use crate::layer::decor::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorOrientationMode, DecorRules};

/// The kind's variants, each a model by its path under asset/ and the skin
/// it is drawn with, in the order the scatter picks among them.
const VARIANTS: [DecorVariantAsset; 5] = [
    DecorVariantAsset {
        model: Asset::decor_kenney_car_kit_sedan_glb,
        skin: Asset::decor_kenney_car_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_car_kit_suv_glb,
        skin: Asset::decor_kenney_car_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_car_kit_van_glb,
        skin: Asset::decor_kenney_car_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_car_kit_hatchback_sports_glb,
        skin: Asset::decor_kenney_car_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_car_kit_taxi_glb,
        skin: Asset::decor_kenney_car_kit_colormap_png,
    },
];

pub const CAR_KIND: DecorKindSeed = DecorKindSeed {
    tag: 0x0ca5eed00003dec0,
    // Much sparser than trees/boats: a highway with a car every couple of
    // cells would read as gridlock, not ambient traffic.
    density: 0.004,
    variant_count: VARIANTS.len() as u32,
    orientation: DecorOrientationMode::AlignTransport,
};

#[allow(non_snake_case)]
pub fn CAR_RULES() -> DecorRules {
    rules_of(&[], &[], &[], &[], &[Transport::road_motorway, Transport::road_primary, Transport::road_secondary].map(|t| t as u32))
}

pub fn is_car_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&CAR_RULES(), land, water, structure, transport)
}

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn CAR_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::Car, CAR_KIND, CAR_RULES(), &VARIANTS, None))
}
