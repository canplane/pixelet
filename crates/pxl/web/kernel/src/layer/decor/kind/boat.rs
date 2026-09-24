// pxl-kernel/layer/decor/kind/boat.rs

//! A decor kind: its seed and rules, and the kind as a bundle, those rules
//! bound to its models.
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

use crate::global::Global;
use crate::layer::decor::build::{bind_models, DecorKindBundle, DecorVariantAsset};
use crate::layer::decor::kind::DecorKind;
use crate::layer::decor::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorOrientationMode, DecorRules};

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

pub const BOAT_KIND: DecorKindSeed = DecorKindSeed {
    tag: 0xb0a75eed0002dec0,
    // Sparser than trees' own density would look right here — most rivers are
    // only a few cells wide, so the same per-cell rate reads far denser on a
    // narrow water strip than it does scattered across a wide green area. Cut
    // to 1/4 of the first-pass value (0.015) after that read as boats covering
    // the water, not scattered on it.
    density: 0.002,
    variant_count: VARIANTS.len() as u32,
    orientation: DecorOrientationMode::Random,
};

#[allow(non_snake_case)]
pub fn BOAT_RULES() -> DecorRules {
    rules_of(
        &[GroundChannelKey::STRUCTURE, GroundChannelKey::TRANSPORT],
        &[],
        &[Water::river, Water::lake].map(|t| t as u32),
        &[],
        &[],
    )
}

pub fn is_boat_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&BOAT_RULES(), land, water, structure, transport)
}

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn BOAT_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::Boat, BOAT_KIND, BOAT_RULES(), &VARIANTS, None))
}
