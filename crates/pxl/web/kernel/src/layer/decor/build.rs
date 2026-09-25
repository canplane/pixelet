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
use pxl_spec::bake::constant::{unpack_pos, U_PER_CELL};
use pxl_spec::cell::object::decor_num_id;
use pxl_spec::cell::DecorId;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{gather, IVec};
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND, PAGE_CELLS};
use xpute_core::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

use std::collections::HashMap;

use pxl_spec::cell::cube_types;

use crate::kernel::kernel;
use crate::layer::decor::instance::{DecorScratch, Instance, DECOR_GROUPS, HIDDEN_FLAG, NO_ENTRY};
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{transport_aligned_rotation, SCALE_JITTER};
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};
use crate::layer::object::store::{At, Store};
use crate::world::chunk::table::Drawn;
use pxl_spec::asset::ingested::Asset;

const AIR: u32 = cube_types::common::air as u32;

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
    pub seed: DecorKindSeed,
    /// Where the kind stands, as the scheduler is taught it.
    pub rules: DecorRules,
    /// One entry per model kind.rs lists for this kind, in its order —
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

/// Binds a kind's seed and rules to its variants, and measures the kind's
/// extent over all of them.
pub fn bind_models(def: &DecorKindDef) -> DecorKindBundle {
    // A NaN extent stays NaN rather than being dropped.
    let extent = def
        .variants
        .iter()
        .flat_map(|v| v.model.ingested().extent.unwrap_or([0.0; 3]))
        .fold(0.0, |a: f64, b| if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) });
    DecorKindBundle {
        seed: def.seed,
        rules: def.rules,
        variants: def.variants.to_vec(),
        decor_id: def.decor_id,
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
        sched.decor_kind(Kind {
            tag: bundle.seed.tag,
            density: bundle.seed.density,
            variant_count: bundle.variants.len() as u32,
            align_transport: bundle.seed.orientation == DecorOrientationMode::AlignTransport,
            extent: bundle.extent,
            decor_value: bundle.decor_id.map(decor_num_id),
            rules: bundle.rules,
        });
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
// at its own level, each cell's candidate the one a z16 cell under it draws
// with a random stream seeded from that cell's page, the kind and the cell
// alone — nothing about the view, so a tree stands in the same place however
// the chunk was reached and whatever level draws it — keeps what lands on
// eligible ground at the density, and stands it on the terrain unless a
// placed object occupies that cell. The user one is every decor entry the
// object store holds in the chunk's columns, turned and varied by a stream
// seeded from its own corner. What comes out is one block: for each kind and
// variant, the instances' lanes.
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
    pub rules: DecorRules,
}

/// A chunk's decor at a tier, gathered and ready to be laid into its block.
pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
}

/// The ground a chunk's decor is scattered over: the chunk's own page at
/// its own level, the cells of it the chunk covers, and their heights.
pub struct Ground<'a> {
    pub page: &'a PageView<'a>,
    pub sample: &'a ChunkSample,
    pub level: u32,
    /// The chunk's corner, in units.
    pub x0: i64,
    pub y0: i64,
    /// Where the chunk starts in its page, in the level's cells.
    pub tx0: u32,
    pub ty0: u32,
    /// The column a z16 chunk stands in, whose placed objects keep a
    /// candidate off the cell they fill. None above z16, where a cell is the
    /// ground of many columns and the objects are folded into the chunk's
    /// cubes rather than standing in its cells.
    pub col_idx: Option<IVec>,
    /// The surface the chunk's tier is drawn with, which an instance stands
    /// on, as every mark stands on the surface drawn under it; the page's
    /// height where none is built yet.
    pub surface: Option<&'a Drawn<'a>>,
    /// Above z16, the chunk's objects folded to its level's cell, as its
    /// runs draw them: a candidate is kept off a cell they fill.
    pub folded: Option<&'a HashMap<[i64; 3], u32>>,
}

/// A column whose placed decor a chunk draws.
pub struct Placed {
    pub col_idx: IVec,
    /// The hidden entry's band and packed position in this column.
    pub hide: Option<(i32, u32)>,
    /// Where the column's corner is from the corner of the chunk drawing it,
    /// in units: zero for a z16 chunk, its place among the columns under it
    /// for a coarser one.
    pub ox: f64,
    pub oy: f64,
    /// Whether a placed instance keeps the entry it came from, for the
    /// eraser's hit. A coarser chunk draws several columns' decor and names
    /// none of their entries: what is erased is erased from close by.
    pub hittable: bool,
}

/// The z16 cell a level cell's candidate is drawn from: at each level down,
/// one of the node's four children, chosen by a stream keyed to the kind and
/// the node alone. So a level's candidate is always the candidate of one of
/// the cells under it, and a zoom in or out keeps the instances it had —
/// each one the instance that z16 cell draws, where it stands, as it turns —
/// and adds or drops the others, a quarter a level.
fn representative(tag: u64, level: u32, x: i64, y: i64) -> (i64, i64) {
    (1..=level).rev().fold((x, y), |(x, y), l| {
        let slot = derive_seed_u64(derive_seed_u64(derive_seed_u64(DECOR_MASTER_SEED, tag), gather(x, y, 0)), l as u64);
        (2 * x + (slot & 1) as i64, 2 * y + (slot >> 1 & 1) as i64)
    })
}

impl<'s> DecorBuild<'s> {
    /// Builds every kind's instances at world step `step` into `scratch`:
    /// the decor placed in `placed`'s columns, and where `show_decor`, the
    /// scatter over `ground`. A chunk scatters from its own level's page, so
    /// decoration reaches as far as the ground it stands on is drawn, a kind
    /// as far as it covers the pixels it is drawn at (`decor_density`).
    #[allow(clippy::too_many_arguments)]
    pub fn build(scratch: &'s mut DecorScratch, kinds: &[Kind], ground: Option<&Ground>, placed: &[Placed], store: &Store, step: u32, show_decor: bool) -> DecorBuild<'s> {
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
            for column in placed {
                Self::placed(scratch, kind, first, column, store);
            }
            if let Some(ground) = ground.filter(|_| show_decor && decor_density(kind.extent, step) > 0.0) {
                Self::scatter(scratch, kind, first, ground, store);
            }
        }
        DecorBuild { scratch }
    }

    /// One column's placed instances of `kind`, into its groups from
    /// `first`, in the store's entry order.
    fn placed(scratch: &mut DecorScratch, kind: &Kind, first: usize, column: &Placed, store: &Store) {
        let (Some(value), Some(e)) = (kind.decor_value, store.entries(column.col_idx)) else { return };
        let (bands, poss, states) = (e.band, e.pos, e.state);
        for k in 0..bands.len() {
            if states[k] != value {
                continue;
            }
            let pos = poss[k];
            let (x, y, z, scale) = unpack_pos(pos);
            let size = scale.side();
            let at = At {
                col_idx: column.col_idx,
                band: bands[k],
                step: [x, y, z],
            };
            // The entry's cell, which seeds its variant and turn.
            let [cx, cy, cz] = at.cell();
            let corner = gather(cx, cy, cz);
            let [lx, ly, lz] = at.local();
            let mut rng = Rng::new(derive_seed_u64(DECOR_MASTER_SEED, corner));
            let variant = ((rng.unit() * kind.variant_count as f64).floor() as u32).min(kind.variant_count - 1);
            let rotation = rng.unit() * core::f64::consts::PI * 2.0;
            let hidden = column.hide == Some((bands[k], pos));
            let h = size / 2.0;
            scratch.push(
                first + variant as usize,
                Instance {
                    x: (column.ox + lx + h) as f32,
                    y: (column.oy + ly + h) as f32,
                    z: lz as f32,
                    rotation: rotation as f32,
                    scale: 1.0,
                    flags: if hidden { HIDDEN_FLAG } else { 0 },
                    band: if column.hittable { bands[k] as u32 } else { NO_ENTRY },
                    pos,
                },
            );
        }
    }

    /// The scatter of `kind` over `ground`: for each of the chunk's cells
    /// the kind may stand on, the candidate of the z16 cell it draws from
    /// (`representative`), with the draws that cell makes at z16 — kept at
    /// the kind's density, stood on the surface drawn there, and left out
    /// where a placed object fills the cell as it is drawn.
    ///
    /// Whether a kind may stand on a cell, and which way a kind aligned to
    /// transport faces, are read off the level's own cells, the ground drawn
    /// there: a tree above z16 stands on the grass the view sees, not on a
    /// z16 cell it cannot, so it can come and go with a level as that grass
    /// does.
    fn scatter(scratch: &mut DecorScratch, kind: &Kind, first: usize, ground: &Ground, store: &Store) {
        let cells = ground.page.cells;
        let (level, atom) = (GROUND.scale(ground.level), GROUND.scale(0));
        let (lx0, ly0) = (level.index(ground.x0), level.index(ground.y0));
        let has_entries = ground.col_idx.is_some_and(|c| store.entries(c).is_some());
        for dy in 0..CHUNK_CELLS {
            let cell_y = ground.ty0 + dy;
            for dx in 0..CHUNK_CELLS {
                let cell_x = ground.tx0 + dx;
                let idx = (cell_y * PAGE_CELLS + cell_x) as usize;
                if !kind.rules.allows(cells[idx]) {
                    continue;
                }
                let (gx, gy) = representative(kind.tag, ground.level, lx0 + dx as i64, ly0 + dy as i64);
                // The z16 cell's own stream: its page's seed, the kind's, and
                // its place in its page.
                let unit = |g: i64| (g as f64 * atom.side()) as i64;
                let page_idx = L3_IDX(gather(unit(gx), unit(gy), 0));
                let (px, py) = (gx.rem_euclid(PAGE_CELLS as i64) as u64, gy.rem_euclid(PAGE_CELLS as i64) as u64);
                let kind_seed = derive_seed_u64(derive_seed_u64(DECOR_MASTER_SEED, page_idx), kind.tag);
                let mut rng = Rng::new(derive_seed_u64(kind_seed, (px << 16) | py));
                if rng.unit() >= kind.density {
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
                let ele = ground.sample.ele[apron_col(dx, dy) as usize] as i32;
                if has_entries && ground.col_idx.is_some_and(|c| store.cell(c, dx, dy, ele).is_some_and(|c| c.solid > 0)) {
                    continue;
                }
                let filled = |f: &HashMap<[i64; 3], u32>| f.get(&[lx0 + dx as i64, ly0 + dy as i64, level.index(ele as i64)]).is_some_and(|&s| s != AIR);
                if ground.folded.is_some_and(filled) {
                    continue;
                }
                let (x, y) = ((gx as f64 + 0.5) * atom.side() - ground.x0 as f64, (gy as f64 + 0.5) * atom.side() - ground.y0 as f64);
                let z = ground.surface.and_then(|d| d.top_under(x, y)).unwrap_or(ele as f64);
                scratch.push(
                    first + variant as usize,
                    Instance {
                        x: x as f32,
                        y: y as f32,
                        z: z as f32,
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
