// pxl-kernel/layer/decor/kind/tree.rs

//! A decor kind: its seed and rules, and the kind as a bundle, those rules
//! bound to its models.
//!
//! Trees on green/forest terrain, and only where nothing else (water, a
//! structure, a road) already claims the cell — the same "what actually
//! renders there" precedence the terrain paint applies, since a cell whose
//! land channel says green_light but whose transport channel is a road is a
//! road, not grass.
//!
//! bare is included alongside the two explicit GREEN_* types even though its
//! own name says "bare ground": it's the fallback land type for any cell
//! with no real landuse tag, and this pack's actual land_bare.png texture is
//! a grassy yellow-green speckle nearly indistinguishable from
//! land_green_light.png — visually it *is* the plain grass most of an
//! untagged map reads as, not dirt. SPORTS was considered too (its own
//! texture leans mint-green) but left out: it's a maintained field, not
//! natural ground, and reads wrong with trees scattered across it.

use pxl_spec::asset::ingested::Asset;
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::cell::cube_types::Land;
use pxl_spec::cell::decor_types::Decor;

use crate::global::Global;
use crate::layer::decor::build::{bind_models, DecorKindBundle, DecorVariantAsset};
use crate::layer::decor::kind::DecorKind;
use crate::layer::decor::scatter::{eligibility_of, rules_of, DecorKindSeed, DecorOrientationMode, DecorRules};

/// The kind's variants, each a model by its path under asset/, in the
/// order the scatter picks among them: the fantasy town kit's five trees
/// and the graveyard kit's four pines.
///
/// Each variant's colormap, row for row with `models`: the Kenney packs keep
/// the texture outside the glb and share it across the pack, and a kind's
/// variants can come from more than one pack.
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

pub const TREE_KIND: DecorKindSeed = DecorKindSeed {
    // Arbitrary, fixed — only has to stay stable across sessions; a big odd
    // constant rather than a small number so it mixes well even before
    // combining with a per-page seed.
    tag: 0x7ee5eed7eec0de1,
    // Moderate, deliberately sparse — a scattered handful of trees reading as
    // "this hillside has some trees on it", not a solid forest wall at 1u
    // spacing (which a much higher density would read as, given every eligible
    // cell is only 1u wide).
    density: 0.04,
    variant_count: VARIANTS.len() as u32,
    orientation: DecorOrientationMode::Random,
};

#[allow(non_snake_case)]
pub fn TREE_RULES() -> DecorRules {
    rules_of(
        &[GroundChannelKey::WATER, GroundChannelKey::STRUCTURE, GroundChannelKey::TRANSPORT],
        &[Land::green_light, Land::green_deep, Land::bare].map(|t| t as u32),
        &[],
        &[],
        &[],
    )
}

pub fn is_tree_eligible(land: u32, water: u32, structure: u32, transport: u32) -> bool {
    eligibility_of(&TREE_RULES(), land, water, structure, transport)
}

static BUNDLE: Global<DecorKindBundle> = Global::new();

#[allow(non_snake_case)]
pub fn TREE_DECOR_BUNDLE() -> &'static DecorKindBundle {
    BUNDLE.get(|| bind_models(DecorKind::Tree, TREE_KIND, TREE_RULES(), &VARIANTS, Some(Decor::tree)))
}
