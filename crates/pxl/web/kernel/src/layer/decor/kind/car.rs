// pxl-kernel/layer/decor/kind/car.rs

//! A decor kind: its seed, its rules and its models, as the registry
//! (decor/registry.rs) binds them.
//!
//! Ambient cars on major roads. Motorway, trunk, primary, secondary and
//! tertiary only, deliberately: ordinary neighborhood streets and pedestrian ways
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
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::cell::cube_types::Transport;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

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

pub const DEF: DecorKindDef = DecorKindDef {
    seed: DecorKindSeed {
        tag: 0x0ca5eed00003dec0,
        // Much sparser than trees/boats: a highway with a car every couple of
        // cells would read as gridlock, not ambient traffic.
        density: 0.004,
        orientation: DecorOrientationMode::AlignTransport,
    },
    rules: DecorRules::ANY.one_of(
        GroundChannelKey::TRANSPORT,
        &[
            Transport::road_motorway as u8,
            Transport::road_trunk as u8,
            Transport::road_primary as u8,
            Transport::road_secondary as u8,
            Transport::road_tertiary as u8,
        ],
    ),
    variants: &VARIANTS,
    decor_id: None,
};
