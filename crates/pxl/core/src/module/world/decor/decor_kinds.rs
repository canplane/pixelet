// pxl_core/module/world/decor/decor_kinds.rs

//! The decor kinds: what real GLB models stand where, taught to the scheduler
//! (crates/pxl/core/src/world/decor.rs) as data — each kind's extent, density,
//! the channels it scatters on, its variants and their palette rows — and the
//! falloff by tier the build uses.
//!
//! No decoration is ever stored: scatter.rs's placement is a pure function of
//! terrain + a fixed seed, recomputed on demand. A cell that currently holds a
//! user-placed object simply has its candidate filtered out at build time;
//! removing that object clears the very same filter on the next rebuild, so the
//! decoration reappears with no bookkeeping of its own.
//!
//! The tier split is Minecraft's own block model, taken literally (2026-09-05):
//! static grid content — cubes and decor alike — bakes into the chunk's own
//! blocks; anything that moves under its own logic every frame (the companion)
//! lives outside the grid as an actor. User-placed decor lands in the object
//! store as its own entry kind alongside solid cubes, and is merged into the
//! build next to the scattered instances, not kept in a separate store.

use pxl_shared::bake::constant::U_PER_CELL;
use pxl_shared::record::palette::PaletteTable;
use pxl_shared::record::shipped::shipped_palette;
use pxl_shared::spec::cell::{decor_num_id, DecorId};
use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::kit::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

use super::scatter::{DecorChannelRule, DecorKindSeed, DecorOrientationMode, DecorRules};
use crate::sys::global::Global;
use crate::sys::kernel::kernel;
use crate::lib::env::route::asset_url;
use crate::module::world::lod::{LodStep, TARGET_CELL_PX};
use crate::world::decor::{Kind as DecorKind, Rule as DecorRule};

// Fixed, arbitrary — the one thing every kind's own placement ultimately
// derives from (alongside its own seed tag). Changing this reshuffles every
// decoration in the world. The TypeScript literal is wider than a word; a seed
// derivation masks to 64 bits, so this is the part of it that ever mattered.
pub const DECOR_MASTER_SEED: u64 = 0xd0c0_dec0_5eed_0001;

/// How far a kind is drawn, from its size and nothing else.
///
/// One criterion for every kind, the terrain tiers' own: what decides whether a
/// thing is drawn is how large it is to the eye. A kind brings its extent — the
/// largest dimension of its models, in world units — and never a threshold. At
/// the far edge of a tier's band, a cell projects to TARGET_CELL_PX; an extent
/// of E cells projects to E times that over the step. A thing that would cover
/// fewer pixels than two of the tier's cells is not drawn: at that size its
/// absence makes no visible difference, and the tier's cubes are already
/// coarser than it. Twelve pixels at the reference viewport — the middle of the
/// range at which Google's map products stop drawing a building in 3D
/// (2026-09-13, decided by eye).
///
/// No fade: a kind is drawn at full density up to its last tier and not past
/// it. What softens the edge is the blend across a band boundary, which
/// staggers the tier change across chunks. Cost is the low-poly tier's problem,
/// not the criterion's; a tree and a car of the same size are drawn to the same
/// range.
fn decor_gone_px() -> f64 {
    TARGET_CELL_PX() * 2.0
}

/// The density a kind is scattered at on a tier: 1, or 0 when it is not drawn
/// there.
pub fn decor_density(extent_u: f64, lod_step: LodStep) -> f64 {
    let projected_px = (extent_u / U_PER_CELL) * TARGET_CELL_PX() / lod_step as f64;
    if projected_px < decor_gone_px() {
        0.0
    } else {
        1.0
    }
}

/// One variant of a kind: its model and the skin it is drawn with.
#[derive(Clone, Debug, PartialEq)]
pub struct DecorVariantAsset {
    pub model_url: String,
    /// Not necessarily shared across a kind's other variants: a kind's variants
    /// may come from different packs with different colormaps.
    pub skin_url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecorKindBundle {
    /// The kind's name — the palette group, and the draw census's tag.
    pub kind: &'static str,
    pub seed: DecorKindSeed,
    /// Where the kind stands, as the scheduler is taught it.
    pub rules: DecorRules,
    /// One entry per variant_count, from the palette's rows for this kind —
    /// each variant carries its own skin: a kind's variants can (and for trees,
    /// do) span more than one pack, each with its own colormap atlas.
    pub variants: Vec<DecorVariantAsset>,
    /// Which object store decor value a user-placed instance of this kind is
    /// tagged with — none for a kind with no user-placeable equivalent yet.
    pub decor_id: Option<DecorId>,
    /// The palette rows of this kind's variants, in variant order.
    pub palette_rows: Vec<u32>,
}

static PALETTE: Global<PaletteTable> = Global::new();

/// The shipped palette, decoded once.
pub fn palette() -> &'static PaletteTable {
    PALETTE.get(shipped_palette)
}

/// Binds a bundle to the palette: its variants — the palette's model rows for
/// this kind, in row order, each a model and a skin fetched by content key.
/// The seed's variant_count is checked against the palette so the scatter's
/// pick and the palette's list cannot drift apart silently.
pub fn bind_palette(kind: &'static str, seed: DecorKindSeed, rules: DecorRules, decor_id: Option<DecorId>) -> DecorKindBundle {
    let p = palette();
    let rows: Vec<u32> = p.rows_of_group(kind).unwrap_or_default().into_iter().filter(|&row| p.model_key_of(row).is_ok_and(|k| !k.is_empty())).collect();
    assert_eq!(rows.len() as u32, seed.variant_count, "decor: palette lists {} models for {kind}, kind.json's variant_count is {}", rows.len(), seed.variant_count);
    let variants = rows
        .iter()
        .map(|&row| DecorVariantAsset {
            model_url: asset_url(&p.model_key_of(row).unwrap_or_default()).to_string(),
            skin_url: asset_url(&p.skin_key_of(row).unwrap_or_default()).to_string(),
        })
        .collect();
    DecorKindBundle {
        kind,
        seed,
        rules,
        variants,
        decor_id,
        palette_rows: rows,
    }
}

/// The extent the range criterion reads: the largest dimension over every
/// variant, since the criterion asks how large the kind is to the eye and a
/// kind is drawn as any of its variants.
pub fn kind_extent(bundle: &DecorKindBundle) -> f64 {
    bundle.palette_rows.iter().map(|&row| palette().extent_max(row) as f64).fold(0.0, f64::max)
}

/// Teaches the scheduler every kind, in bundle order — the order of the groups
/// in the decor blocks it builds: each kind's variants in variant order, one
/// group a variant.
pub fn teach_decor_kinds(bundles: &[&DecorKindBundle]) {
    let sched = &mut kernel().sched;
    sched.decor_forget();
    for bundle in bundles {
        let idx = sched.decor_kind(DecorKind {
            tag: bundle.seed.tag,
            density: bundle.seed.density,
            variant_count: bundle.seed.variant_count,
            align_transport: bundle.seed.orientation == DecorOrientationMode::AlignTransport,
            extent: kind_extent(bundle),
            decor_value: bundle.decor_id.map(decor_num_id),
            rules: [DecorRule::Any; 4],
        });
        for ch in 0..4 {
            match bundle.rules.channel(ch) {
                DecorChannelRule::Any => {}
                DecorChannelRule::Void => sched.decor_rule(idx, ch, DecorRule::Void),
                DecorChannelRule::OneOf(types) => {
                    sched.decor_rule(idx, ch, DecorRule::OneOf([0; 4]));
                    for &t in types {
                        sched.decor_allow(idx, ch, t as u8);
                    }
                }
            }
        }
    }
}

// Seeded purely by the placed object's own absolute position — there's no
// stable "page seed" to key a user placement off: the object itself is the one
// stable identity a placement keeps across reloads. The hover ghost calls this
// too, at the exact same `i` a placement resolves to, so the ghost shows the
// real variant/heading a real placement would get — this being pure and keyed
// only on `i` is exactly what makes that prediction exact rather than a guess.
pub fn user_decor_pick(i: IVec, variant_count: u32) -> (u32, f64) {
    let mut stream = SplitMix64::new(derive_seed_u64(DECOR_MASTER_SEED, i));
    let mut rng = || stream.next_u32();
    let variant = ((u32_to_unit(rng()) * variant_count as f64).floor() as u32).min(variant_count - 1);
    let rotation = u32_to_unit(rng()) * core::f64::consts::PI * 2.0;
    (variant, rotation)
}
