// pxl-kernel/layer/decor/kind/tree.rs

//! Trees on green and bare land. Bare is the fallback type for untagged land
//! and this pack draws it as grass; sports fields are left out as maintained,
//! not natural, ground.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::cell::cube_types::Land;
use pxl_spec::cell::decor_types::Decor;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

/// Each variant's colormap is per pack, since Kenney keeps textures outside the glb.
const VARIANTS: [DecorVariantAsset; 9] = [
    DecorVariantAsset {
        model: Asset::decor_kenney_fantasy_town_tree_glb,
        skin: Asset::decor_kenney_fantasy_town_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_fantasy_town_tree_high_glb,
        skin: Asset::decor_kenney_fantasy_town_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_fantasy_town_tree_crooked_glb,
        skin: Asset::decor_kenney_fantasy_town_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_fantasy_town_tree_high_round_glb,
        skin: Asset::decor_kenney_fantasy_town_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_fantasy_town_tree_high_crooked_glb,
        skin: Asset::decor_kenney_fantasy_town_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_graveyard_kit_pine_glb,
        skin: Asset::decor_kenney_graveyard_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_graveyard_kit_pine_crooked_glb,
        skin: Asset::decor_kenney_graveyard_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_graveyard_kit_pine_fall_glb,
        skin: Asset::decor_kenney_graveyard_kit_colormap_png,
    },
    DecorVariantAsset {
        model: Asset::decor_kenney_graveyard_kit_pine_fall_crooked_glb,
        skin: Asset::decor_kenney_graveyard_kit_colormap_png,
    },
];

pub const DEF: DecorKindDef = DecorKindDef {
    seed: DecorKindSeed {
        // Arbitrary; changing it reshuffles every tree.
        tag: 0x7ee5eed7eec0de1,
        // Sparse: a hillside with some trees, not a forest wall at 1u spacing.
        density: 0.04,
        orientation: DecorOrientationMode::Random,
    },
    rules: DecorRules::on(GroundChannelKey::LAND, &[Land::green_light as u8, Land::green_deep as u8, Land::bare as u8]),
    variants: &VARIANTS,
    decor_id: Some(Decor::tree),
};
