// pxl-kernel/layer/decor/kind/boat.rs

//! A decor kind: its seed, its rules and its models, as the registry
//! (decor/registry.rs) binds them.
//!
//! Small boats on open water. Open river/lake water only — WETLAND is
//! excluded (too shallow/marshy for a boat to sit in), and
//! structure/transport must both still be VOID: per the terrain paint's
//! precedence, either one (a pier, a bridge deck) would win over water and
//! actually render as itself, not water, at that cell — a boat there would
//! visually float on top of a bridge instead of the water beneath it.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::cell::cube_types::Water;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

/// The kind's variants, each a model by its path under asset/ and the skin
/// it is drawn with, in the order the scatter picks among them.
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

pub const DEF: DecorKindDef = DecorKindDef {
    seed: DecorKindSeed {
        tag: 0xb0a75eed0002dec0,
        // Sparser than trees' own density would look right here — most rivers are
        // only a few cells wide, so the same per-cell rate reads far denser on a
        // narrow water strip than it does scattered across a wide green area. Cut
        // to 1/4 of the first-pass value (0.015) after that read as boats covering
        // the water, not scattered on it.
        density: 0.002,
        orientation: DecorOrientationMode::Random,
    },
    rules: DecorRules::ANY
        .void(GroundChannelKey::STRUCTURE)
        .void(GroundChannelKey::TRANSPORT)
        .one_of(GroundChannelKey::WATER, &[Water::river as u8, Water::lake as u8]),
    variants: &VARIANTS,
    decor_id: None,
};
