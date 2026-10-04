// pxl-kernel/layer/decor/kind/train.rs

//! Trains on rail lines the ground shows (a tunnel's is not painted), aligned
//! to the rail, since one lying across it reads as derailed.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::cell::cube_types::Transport;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

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

pub const DEF: DecorKindDef = DecorKindDef {
    seed: DecorKindSeed {
        tag: 0x7ba15eed0004dec0,
        // Very sparse: trains should read as occasional.
        density: 0.004,
        orientation: DecorOrientationMode::AlignTransport,
    },
    rules: DecorRules::ANY.one_of(GroundChannelKey::TRANSPORT, &[Transport::rail_main as u8]),
    variants: &VARIANTS,
    decor_id: None,
};
