// pxl-kernel/layer/decor/kind/boat.rs

//! Boats on open river and lake water; wetland is too shallow.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::cell::cube_types::ground as g;
use pxl_spec::cell::decor_types::Decor;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::Kind;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

const VARIANTS: [DecorVariantAsset; 4] = [
    DecorVariantAsset {
        model: Asset::decor_kenney_watercraft_pack_boat_row_small_glb,
        skin: Asset::decor_kenney_watercraft_pack_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_watercraft_pack_boat_fishing_small_glb,
        skin: Asset::decor_kenney_watercraft_pack_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_watercraft_pack_boat_sail_a_glb,
        skin: Asset::decor_kenney_watercraft_pack_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_watercraft_pack_boat_speed_a_glb,
        skin: Asset::decor_kenney_watercraft_pack_colormap_png,
    },
];

pub const DEF: Kind = Kind {
    seed: DecorKindSeed {
        tag: 0xb0a75eed0002dec0,
        // Rivers are a few cells wide, so the per-cell rate reads far denser than on
        // land: cut to a quarter of 0.015 after boats covered the water.
        density: 0.002,
        orientation: DecorOrientationMode::Random,
    },
    rules: DecorRules::on(&[g::water]),
    variants: &VARIANTS,
    decor_id: Some(Decor::boat),
};
