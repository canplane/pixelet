// pxl-kernel/layer/decor/kind/train.rs

//! A decor kind: its seed, its rules and its models, as the registry
//! (decor/registry.rs) binds them.
//!
//! Occasional trains on rail lines, subway included: what runs in a tunnel
//! is not painted on the ground at all, so a subway track the ground shows is
//! one at the surface. Same "align_transport" reasoning as car applies: a
//! train lying across the rail sideways would read as derailed. It stands
//! where the cell shows rail (scatter.rs `DecorRules`).

use pxl_spec::asset::ingested::Asset;
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::cell::cube_types::Transport;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

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

pub const DEF: DecorKindDef = DecorKindDef {
    seed: DecorKindSeed {
        tag: 0x7ba15eed0004dec0,
        // Very sparse, deliberately — trains are meant to read as occasional, not
        // as a train sitting on every visible stretch of track.
        density: 0.004,
        orientation: DecorOrientationMode::AlignTransport,
    },
    rules: DecorRules::on(GroundChannelKey::TRANSPORT, &[Transport::rail_main as u8, Transport::rail_subway as u8]),
    variants: &VARIANTS,
    decor_id: None,
};
