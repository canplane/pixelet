// pxl-kernel/layer/decor/build.rs

//! The decor layer: what real GLB models stand where. The first half is the
//! kinds, taught to the scheduler as data — each kind's extent, density, the
//! channels it scatters on and its variants — and the falloff by tier the
//! build uses; the second is the build, a chunk's decor at a tier.
//!
//! No decoration is ever stored: scatter.rs's placement is a pure function of
//! terrain + a fixed seed, recomputed on demand. A cell that currently holds a
//! user-placed object simply has its candidate filtered out at build time;
//! removing that object clears the very same filter on the next rebuild, so the
//! decoration reappears with no bookkeeping of its own.
//!
//! The tier split is Minecraft's own block model, taken literally:
//! static grid content — cubes and decor alike — bakes into the chunk's own
//! blocks; anything that moves under its own logic every frame (the companion)
//! lives outside the grid as an actor. User-placed decor lands in the object
//! store as its own entry kind alongside solid cubes, and is merged into the
//! build next to the scattered instances, not kept in a separate store.

use pxl_render::gpu::api::word_bytes;
use pxl_render::look::lod::{LodStep, TARGET_CELL_PX};
use pxl_render::mesh::paint::terrain::adapter::{ChunkSample, PageView};
use pxl_render::mesh::voxel_scan::apron_col;
use pxl_spec::bake::constant::{CHUNK_CELL_SPAN, PAGE_CELL_SPAN, U_PER_CELL};
use pxl_spec::bake::pack::unpack;
use pxl_spec::cell::object::decor_num_id;
use pxl_spec::cell::DecorId;
use pxl_spec::space::pvf3::ivec::{gather, IVec};
use xpute_core::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

use crate::kernel::kernel;
use crate::layer::decor::instance::{DecorScratch, Instance, DECOR_GROUPS, HIDDEN_FLAG, NO_ENTRY};
use crate::layer::decor::kind::spec::DecorKind;
use crate::layer::decor::scatter::{transport_aligned_rotation, SCALE_JITTER};
use crate::layer::decor::scatter::{DecorChannelRule, DecorKindSeed, DecorOrientationMode, DecorRules};
use crate::layer::object::store::{unpack_pos, Store, BAND_CELLS, CELL_STEPS, STEP_UNITS};
use pxl_spec::asset::ingested::Asset;

// Fixed, arbitrary — the one thing every kind's own placement ultimately
// derives from (alongside its own seed tag). Changing this reshuffles every
// decoration in the world. A seed derivation masks to 64 bits.
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
/// (decided by eye).
///
/// No fade: a kind is drawn at full density up to its last tier and not past
/// it. What softens the edge is the blend across a band boundary, which
/// staggers the tier change across chunks. Cost is the low-poly tier's problem,
/// not the criterion's; a tree and a car of the same size are drawn to the same
/// range.
fn decor_gone_px() -> f64 {
    *TARGET_CELL_PX * 2.0
}

/// The density a kind is scattered at on a tier: 1, or 0 when it is not drawn
/// there.
pub fn decor_density(extent_u: f64, lod_step: LodStep) -> f64 {
    let projected_px = (extent_u / U_PER_CELL) * *TARGET_CELL_PX / lod_step as f64;
    if projected_px < decor_gone_px() {
        0.0
    } else {
        1.0
    }
}

/// One variant of a kind: its model and the skin it is drawn with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorVariantAsset {
    pub model: Asset,
    /// Not necessarily shared across a kind's other variants: a kind's variants
    /// may come from different packs with different colormaps.
    pub skin: Asset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecorKindBundle {
    /// The kind: kind.json's entry, and the draw census's tag.
    pub kind: DecorKind,
    pub seed: DecorKindSeed,
    /// Where the kind stands, as the scheduler is taught it.
    pub rules: DecorRules,
    /// One entry per model kind.json lists for this kind, in its order —
    /// each variant carries its own skin: a kind's variants can (and for trees,
    /// do) span more than one pack, each with its own colormap atlas.
    pub variants: Vec<DecorVariantAsset>,
    /// Which object store decor value a user-placed instance of this kind is
    /// tagged with — none for a kind with no user-placeable equivalent yet.
    pub decor_id: Option<DecorId>,
    /// The largest dimension over every variant's model, in world units: what
    /// the range criterion reads, since it asks how large the kind is to the eye
    /// and a kind is drawn as any of its variants.
    pub extent: f64,
}

/// Binds a kind's seed and rules to its models: each variant a model and a
/// skin, `models` and `skins` row for row as kind.json lists them, and the
/// kind's extent measured over all of them.
pub fn bind_models(kind: DecorKind, seed: DecorKindSeed, rules: DecorRules, models: &[Asset], skins: &[Asset], decor_id: Option<DecorId>) -> DecorKindBundle {
    assert_eq!(models.len(), skins.len(), "decor: kind.json lists {} models for {kind:?} and {} skins", models.len(), skins.len());
    let variants = models.iter().zip(skins).map(|(&model, &skin)| DecorVariantAsset { model, skin }).collect();
    // A NaN extent stays NaN rather than being dropped.
    let extent = models
        .iter()
        .flat_map(|m| m.ingested().extent.unwrap_or([0.0; 3]))
        .fold(0.0, |a: f64, b| if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) });
    DecorKindBundle {
        kind,
        seed,
        rules,
        variants,
        decor_id,
        extent,
    }
}

/// Teaches the scheduler every kind, in bundle order — the order of the groups
/// in the decor blocks it builds: each kind's variants in variant order, one
/// group a variant.
pub fn teach_decor_kinds(bundles: &[&DecorKindBundle]) {
    let sched = &mut kernel().sched;
    sched.decor_forget();
    for bundle in bundles {
        let idx = sched.decor_kind(Kind {
            tag: bundle.seed.tag,
            density: bundle.seed.density,
            variant_count: bundle.seed.variant_count,
            align_transport: bundle.seed.orientation == DecorOrientationMode::AlignTransport,
            extent: bundle.extent,
            decor_value: bundle.decor_id.map(decor_num_id),
            rules: [Rule::Any; 4],
        });
        for ch in 0..4 {
            match bundle.rules.channel(ch) {
                DecorChannelRule::Any => {}
                DecorChannelRule::Void => sched.decor_rule(idx, ch, Rule::Void),
                DecorChannelRule::OneOf(types) => {
                    sched.decor_rule(idx, ch, Rule::OneOf([0; 4]));
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

// ---- the layer as the kernel builds it ----
//
// Two halves. The procedural one scatters each kind over the chunk's cells
// with a random stream seeded from the page, the kind and the cell alone —
// nothing about the view, so a tree stands in the same place however the
// chunk was reached — keeps what lands on eligible ground at the density,
// and
// stands it on the terrain unless a placed object occupies that cell. The
// user one is every decor entry the object store holds in the column,
// turned and varied by a stream seeded from its own corner. What comes out
// is one block: for each kind and variant, the instances' lanes.
//
// A kind is drawn on a tier only where it is at least two of the tier's
// cells across (decor_density).

/// A decor stream: SplitMix64, drawn as units in [0, 1).
struct Rng(SplitMix64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(SplitMix64::new(seed))
    }

    fn unit(&mut self) -> f64 {
        u32_to_unit(self.0.next_u32())
    }
}

/// What a channel's type must be for a kind to stand on the cell.
#[derive(Clone, Copy)]
pub enum Rule {
    Any,
    Void,
    /// One of the types whose bits are set.
    OneOf([u64; 4]),
}

impl Rule {
    fn allows(&self, t: u8) -> bool {
        match self {
            Rule::Any => true,
            Rule::Void => t == 0,
            Rule::OneOf(bits) => bits[(t >> 6) as usize] & (1 << (t & 63)) != 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Kind {
    pub tag: u64,
    pub density: f64,
    pub variant_count: u32,
    pub align_transport: bool,
    /// The largest dimension of the kind's models, in world units.
    pub extent: f64,
    /// The object store's value a placed instance of this kind carries.
    pub decor_value: Option<u32>,
    /// Land, water, structure, transport.
    pub rules: [Rule; 4],
}

/// A chunk's decor at a tier, gathered and ready to be laid into its block.
pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
}

/// What a chunk's decor is built from.
pub struct Chunk<'a> {
    pub page: &'a PageView<'a>,
    pub page_idx: IVec,
    /// The column this chunk stands in, and its X and Y lanes.
    pub col_idx: IVec,
    pub col_x: i64,
    pub col_y: i64,
    pub tx0: u32,
    pub ty0: u32,
    pub show_decor: bool,
    /// The hidden entry's band and packed position in this column.
    pub hide: Option<(i32, u32)>,
    /// Where the column's corner is from the corner of the chunk drawing it,
    /// in units: zero for a leaf chunk, its place among the columns under it
    /// for a level chunk.
    pub ox: f64,
    pub oy: f64,
    /// Whether a placed instance keeps the entry it came from, for the
    /// eraser's hit. A level chunk draws several columns' decor and names
    /// none of their entries: what is erased is erased from close by.
    pub hittable: bool,
}

impl<'s> DecorBuild<'s> {
    /// Builds every kind's instances at world step `step` into `scratch`,
    /// from `columns` — the leaf columns a chunk covers, each with the sample
    /// already read for it (its elevations). Decoration is scattered from the
    /// leaves alone, whatever level draws the ground under it
    /// (issue/20260921-level-step-and-zoom-are-one-number.md): a column's page
    /// comes from the chunk builder's `leaf_source`, which reads the leaves'
    /// table and no other, and every level's page is the same shape, so it is
    /// that and not a size that holds it.
    pub fn build(scratch: &'s mut DecorScratch, kinds: &[Kind], columns: &[(Chunk, &ChunkSample)], store: &Store, step: u32) -> DecorBuild<'s> {
        scratch.clear();
        for kind in kinds {
            // A kind with no variant has no group to fill and no variant to
            // pick below; the groups are the kinds' variants in order.
            if kind.variant_count == 0 {
                continue;
            }
            // The kind's variants take the next groups; past what a block
            // holds, the kinds after have none.
            let first = scratch.group_count;
            if first + kind.variant_count as usize > DECOR_GROUPS {
                break;
            }
            scratch.group_count += kind.variant_count as usize;
            for (chunk, sample) in columns {
                Self::column(scratch, kind, first, chunk, sample, store, step);
            }
        }
        DecorBuild { scratch }
    }

    /// One column's instances of `kind`, into its groups from `first`.
    fn column(scratch: &mut DecorScratch, kind: &Kind, first: usize, chunk: &Chunk, sample: &ChunkSample, store: &Store, step: u32) {
        let page_seed = derive_seed_u64(DECOR_MASTER_SEED, chunk.page_idx);
        let lanes = store.chunk_lanes(chunk.col_idx);
        // A chunk with no entry has no cell to look up: most chunks.
        let has_entries = lanes.as_ref().is_some_and(|l| l.count > 0);
        {
            // Placed instances first, in the store's entry order.
            if let (Some(value), Some(l)) = (kind.decor_value, &lanes) {
                // SAFETY: the column's lanes, read before anything writes the store.
                let (bands, poss, states) = unsafe {
                    let n = l.count as usize;
                    (
                        core::slice::from_raw_parts(l.band_at, n),
                        core::slice::from_raw_parts(l.pos_at, n),
                        core::slice::from_raw_parts(l.state_at, n),
                    )
                };
                for k in 0..bands.len() {
                    if states[k] != value {
                        continue;
                    }
                    let pos = poss[k];
                    let (x, y, z, level) = unpack_pos(pos);
                    let size = 2f64.powi(level);
                    let corner = gather(
                        chunk.col_x + (x / CELL_STEPS) as i64,
                        chunk.col_y + (y / CELL_STEPS) as i64,
                        bands[k] as i64 * BAND_CELLS as i64 + (z / CELL_STEPS) as i64,
                    );
                    let mut rng = Rng::new(derive_seed_u64(DECOR_MASTER_SEED, corner));
                    let variant = ((rng.unit() * kind.variant_count as f64).floor() as u32).min(kind.variant_count - 1);
                    let rotation = rng.unit() * core::f64::consts::PI * 2.0;
                    let hidden = chunk.hide == Some((bands[k], pos));
                    let h = size / 2.0;
                    scratch.push(
                        first + variant as usize,
                        Instance {
                            x: (chunk.ox + x as f64 * STEP_UNITS + h) as f32,
                            y: (chunk.oy + y as f64 * STEP_UNITS + h) as f32,
                            z: (bands[k] as f64 * BAND_CELLS as f64 + z as f64 * STEP_UNITS) as f32,
                            rotation: rotation as f32,
                            scale: 1.0,
                            flags: if hidden { HIDDEN_FLAG } else { 0 },
                            band: if chunk.hittable { bands[k] as u32 } else { NO_ENTRY },
                            pos,
                        },
                    );
                }
            }

            // Then the scatter, where the kind is drawn on this tier.
            let d = decor_density(kind.extent, step);
            if chunk.show_decor && d > 0.0 {
                let kind_seed = derive_seed_u64(page_seed, kind.tag);
                let cells = chunk.page.cells;
                for dy in 0..CHUNK_CELL_SPAN {
                    let cell_y = chunk.ty0 + dy;
                    for dx in 0..CHUNK_CELL_SPAN {
                        let cell_x = chunk.tx0 + dx;
                        let idx = (cell_y * PAGE_CELL_SPAN + cell_x) as usize;
                        let types = unpack(cells[idx]);
                        if !(0..4).all(|c| kind.rules[c].allows(types[c])) {
                            continue;
                        }
                        let mut rng = Rng::new(derive_seed_u64(kind_seed, ((cell_x as u64) << 16) | cell_y as u64));
                        if rng.unit() >= kind.density * d {
                            continue;
                        }
                        let variant = ((rng.unit() * kind.variant_count as f64).floor() as u32).min(kind.variant_count - 1);
                        let rotation = if kind.align_transport {
                            let flip = rng.unit() < 0.5;
                            transport_aligned_rotation(cells, cell_x, cell_y, flip)
                        } else {
                            rng.unit() * core::f64::consts::PI * 2.0
                        };
                        let scale = 1.0 + (rng.unit() * 2.0 - 1.0) * SCALE_JITTER;
                        let ele = sample.ele[apron_col(dx, dy) as usize] as i32;
                        if has_entries && store.cell(chunk.col_idx, dx, dy, ele).is_some_and(|c| c.solid > 0) {
                            continue;
                        }
                        scratch.push(
                            first + variant as usize,
                            Instance {
                                x: (chunk.ox + dx as f64 + 0.5) as f32,
                                y: (chunk.oy + dy as f64 + 0.5) as f32,
                                z: ele as f32,
                                rotation: rotation as f32,
                                scale: scale as f32,
                                flags: 0,
                                band: NO_ENTRY,
                                pos: 0,
                            },
                        );
                    }
                }
            }
        }
    }

    /// The block's size in bytes: a word of group count, a word of count a
    /// group, then each group's lanes — five of f32, the flags padded to a
    /// word, the band and the position.
    pub fn bytes(&self) -> u32 {
        let s = self.scratch;
        let words: usize = 1 + s.group_count + s.counts[..s.group_count].iter().map(|&n| 7 * n as usize + (n as usize).div_ceil(4)).sum::<usize>();
        word_bytes(words)
    }

    /// Writes the block: each group's lanes at the group's place, each
    /// instance at its rank among its group's, in the order they were made.
    ///
    /// # Safety
    /// `at` must address `bytes()` bytes, 4-aligned, the caller's to write.
    pub unsafe fn write(&self, at: *mut u8) {
        let s = self.scratch;
        let words = at as *mut u32;
        let groups = s.group_count;
        *words = groups as u32;
        let mut first = [0usize; DECOR_GROUPS];
        let mut k = 1 + groups;
        for (g, start) in first.iter_mut().enumerate().take(groups) {
            let n = s.counts[g] as usize;
            *words.add(1 + g) = n as u32;
            *start = k;
            // The flags are bytes padded to a word, and a pad byte is zero.
            core::ptr::write_bytes(at.add((k + 5 * n) * 4), 0, n.div_ceil(4) * 4);
            k += 7 * n + n.div_ceil(4);
        }
        let mut rank = [0usize; DECOR_GROUPS];
        for (i, &g) in s.instances[..s.len].iter().zip(&s.groups[..s.len]) {
            let g = g as usize;
            let (n, r, f) = (s.counts[g] as usize, rank[g], first[g]);
            for (lane, v) in [i.x, i.y, i.z, i.rotation, i.scale].into_iter().enumerate() {
                *words.add(f + lane * n + r) = v.to_bits();
            }
            *at.add((f + 5 * n) * 4 + r) = i.flags;
            let past_flags = f + 5 * n + n.div_ceil(4);
            *words.add(past_flags + r) = i.band;
            *words.add(past_flags + n + r) = i.pos;
            rank[g] += 1;
        }
    }
}

#[cfg(test)]
#[path = "build.test.rs"]
mod test;
