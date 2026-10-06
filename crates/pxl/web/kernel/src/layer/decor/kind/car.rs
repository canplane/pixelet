// pxl-kernel/layer/decor/kind/car.rs

//! Cars on major roads only: neighborhood streets and pedestrian ways are
//! where users build, and a car there reads as in the way. Aligned to the
//! road, since a random heading reads as a crash.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::cell::cube_types::ground as g;
use pxl_spec::cell::decor_types::Decor;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::Kind;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

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

pub const DEF: Kind = Kind {
    seed: DecorKindSeed {
        tag: 0x0ca5eed00003dec0,
        // Sparse: a car every couple of cells reads as gridlock.
        density: 0.004,
        orientation: DecorOrientationMode::AlignTransport,
    },
    rules: DecorRules::on(&[g::motorway, g::trunk, g::primary, g::secondary, g::residential_road]),
    variants: &VARIANTS,
    decor_id: Some(Decor::car),
};
