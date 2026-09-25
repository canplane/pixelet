// pxl-kernel/layer/decor/kind/tree.rs

//! A decor kind: its seed, its rules and its models, as the registry
//! (decor/registry.rs) binds them.
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

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

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

pub const DEF: DecorKindDef = DecorKindDef {
    seed: DecorKindSeed {
        // Arbitrary, fixed — only has to stay stable across sessions; a big odd
        // constant rather than a small number so it mixes well even before
        // combining with a per-page seed.
        tag: 0x7ee5eed7eec0de1,
        // Moderate, deliberately sparse — a scattered handful of trees reading as
        // "this hillside has some trees on it", not a solid forest wall at 1u
        // spacing (which a much higher density would read as, given every eligible
        // cell is only 1u wide).
        density: 0.04,
        orientation: DecorOrientationMode::Random,
    },
    rules: DecorRules::ANY
        .void(GroundChannelKey::WATER)
        .void(GroundChannelKey::STRUCTURE)
        .void(GroundChannelKey::TRANSPORT)
        .one_of(GroundChannelKey::LAND, &[Land::green_light as u8, Land::green_deep as u8, Land::bare as u8]),
    variants: &VARIANTS,
    decor_id: Some(Decor::tree),
};
