// pxl-kernel/layer/decor/build.rs

//! The decor layer: what real GLB models stand where. The first half is the
//! kinds, taught to the scheduler as data — each kind's extent, density, the
//! channels it scatters on and its variants — and the falloff by tier the
//! build uses; the second is the build, a decor cell's population
//! (layer/decor/cell.rs).
//!
//! No decoration is ever stored: scatter.rs's placement is a pure function of
//! terrain + a fixed seed, recomputed on demand. A cell that currently holds a
//! user-placed object simply has its candidate filtered out at build time;
//! removing that object clears the very same filter on the next rebuild, so the
//! decoration reappears with no bookkeeping of its own.
//!
//! The split is Minecraft's own block model, taken literally: static grid
//! content stays on the grid — cubes baked into the chunks, decor into its
//! own cells' blocks — and anything that moves under its own logic every
//! frame (the companion) lives outside the grid as an actor. User-placed decor lands in the object
//! store as its own entry kind alongside solid cubes, and is merged into the
//! build next to the scattered instances, not kept in a separate store.

use pxl_render::gpu::api::word_bytes;
use pxl_render::look::lod::{LodStep, TARGET_CELL_PX};
use pxl_render::mesh::paint::terrain::adapter::{ChunkSample, PageView};
use pxl_render::mesh::voxel_scan::apron_col;
use pxl_spec::bake::constant::{unpack_pos, U_PER_CELL};
use pxl_spec::cell::object::decor_num_id;
use pxl_spec::cell::DecorId;
use pxl_spec::space::hierarchy::child_of;
use pxl_spec::space::pvf3::ivec::{gather, IVec};
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND};
use xpute_core::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

use std::collections::{HashMap, TryReserveError};

use pxl_spec::cell::cube_types;

use crate::global::Global;
use crate::layer::decor::instance::{DecorScratch, GroupLanes, Instance, DECOR_GROUPS, HIDDEN_FLAG, NO_ENTRY};
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{transport_aligned_rotation, SCALE_JITTER};
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};
use crate::layer::object::store::{At, EntryAt, Store};
use crate::world::access::{Access, FINEST};
use crate::world::pyramid::Layer as _;
use pxl_spec::asset::ingested::Asset;
use pxl_spec::bake::constant::CELL;
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pvf3::pvf3::Pvf3;

const AIR: u32 = cube_types::common::air as u32;

// Fixed, arbitrary — the one thing every kind's own placement ultimately
// derives from (alongside its own seed tag). Changing this reshuffles every
// decoration in the world. A seed derivation masks to 64 bits.
pub const DECOR_MASTER_SEED: u64 = 0xd0c0_dec0_5eed_0001;

/// How far a kind is drawn as its model, from its size and nothing else.
///
/// One criterion for every kind, the terrain tiers' own: what decides whether a
/// thing is drawn is how large it is to the eye. A kind brings its extent — the
/// largest dimension of its models, in world units — and never a threshold. At
/// the far edge of a tier's band, a cell projects to TARGET_CELL_PX; an extent
/// of E cells projects to E times that over the step. A thing that would cover
/// fewer pixels than two of the tier's cells is not drawn as a model: at that
/// size its triangles make no visible difference, and the tier's cubes are
/// already coarser than it. Its sprite stands in past that, down to
/// `IMPOSTOR_GONE_CELLS` (`decor_tier`). Twelve pixels at the reference viewport — the middle of the
/// range at which Google's map products stop drawing a building in 3D
/// (decided by eye).
///
/// No fade: a kind is drawn at full density up to its last tier and not past
/// it. The edge is decided per instance, by its own extent at its own
/// distance (layer/decor/cell.rs `present`), so it is staggered instance by
/// instance rather than drawn as a line. Cost is the low-poly tier's problem,
/// not the criterion's; a tree and a car of the same size are drawn to the same
/// range.
pub fn decor_gone_px() -> f64 {
    *TARGET_CELL_PX * 2.0
}

/// How a kind is drawn on a tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorTier {
    /// Its variants' models.
    Model,
    /// Its variants' sprites, a quad an instance (pxl-render
    /// gpu/pass/impostor.rs): too small on the screen for a model's
    /// triangles, large enough to be seen.
    Impostor,
    /// Not at all.
    Gone,
}

/// How a kind of `extent_u` is drawn on a tier of `lod_step`, by how large it
/// is to the eye there: its model down to the model's floor, its sprite down
/// to the impostor's (param.rs `IMPOSTOR_GONE_CELLS`), nothing past.
pub fn decor_tier(extent_u: f64, lod_step: LodStep) -> DecorTier {
    let projected_px = (extent_u / U_PER_CELL) * *TARGET_CELL_PX / lod_step as f64;
    if projected_px >= decor_gone_px() {
        DecorTier::Model
    } else if projected_px >= *TARGET_CELL_PX * crate::param::IMPOSTOR_GONE_CELLS {
        DecorTier::Impostor
    } else {
        DecorTier::Gone
    }
}

/// The density a kind is scattered at on a tier: 1 where it is drawn there
/// at all, model or sprite, 0 where it is not.
pub fn decor_density(extent_u: f64, lod_step: LodStep) -> f64 {
    if decor_tier(extent_u, lod_step) == DecorTier::Gone {
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

/// The largest of `values`, a NaN kept rather than dropped.
fn largest(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, |a: f64, b| if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) })
}

/// The largest dimension of a variant's model, in world units: how large the
/// variant is to the eye, which is what decides how an instance of it is
/// drawn (layer/decor/cell.rs `present`).
pub fn variant_extent(v: &DecorVariantAsset) -> f64 {
    largest(v.model.ingested().extent.unwrap_or([0.0; 3]).into_iter())
}

/// Binds a kind's seed and rules to its variants, and measures the kind's
/// extent over all of them.
pub fn bind_models(def: &DecorKindDef) -> DecorKindBundle {
    let extent = largest(def.variants.iter().map(variant_extent));
    DecorKindBundle {
        seed: def.seed,
        rules: def.rules,
        variants: def.variants.to_vec(),
        decor_id: def.decor_id,
        extent,
    }
}

static KINDS: Global<Vec<Kind>> = Global::new();

/// The decor kinds, in the order they were taught — the order of the groups
/// in every decor block.
pub fn kinds() -> &'static [Kind] {
    KINDS.get(Vec::new)
}

/// Teaches every kind, in bundle order — the order of the groups in the
/// decor blocks: each kind's variants in variant order, one group a variant.
pub fn teach_decor_kinds(bundles: &[&DecorKindBundle]) {
    let kinds = KINDS.get(Vec::new);
    kinds.clear();
    for bundle in bundles {
        kinds.push(Kind {
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
// Two halves. The procedural one scatters each kind over the decor cell's
// cells at its own level, each cell's candidate the one a z16 cell under it
// draws with a random stream seeded from that cell's page, the kind and the
// cell alone — nothing about the view, so a tree stands in the same place
// however the cell was reached and whatever level draws it — keeps what
// lands on eligible ground at the density, and stands it on the terrain
// unless a placed object occupies that cell. The user one is every decor
// entry the object store holds in the decor cell's columns, turned and
// varied by a stream seeded from its own corner. What comes out is one
// block: for each kind and variant, the instances' lanes.
//
// A kind is scattered on a tier only where it is drawn there at all, as a
// model or a sprite (decor_density).

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

/// A decor cell's population, gathered and ready to be laid into its block.
pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
}

/// The ground a decor cell is scattered over: its own page at its own
/// level, the cells of it the decor cell covers, and their heights.
pub struct Ground<'a> {
    pub page: &'a PageView<'a>,
    pub sample: &'a ChunkSample,
    pub level: u32,
    /// The decor cell's corner, in units.
    pub x0: i64,
    pub y0: i64,
    /// Where the decor cell starts in its page, in the level's cells.
    pub tx0: u32,
    pub ty0: u32,
    /// The column a z16 decor cell stands in, whose placed objects keep a
    /// candidate off the cell they fill. None above z16, where a cell is the
    /// ground of many columns and the objects are read folded (`folded`).
    pub col_idx: Option<IVec>,
    /// Above z16, the decor cell's objects folded to its level's cell, as
    /// the chunks' runs draw them: a candidate is kept off a cell they fill.
    pub folded: Option<&'a HashMap<[i64; 3], u32>>,
}

/// A column whose placed decor a decor cell draws.
pub struct Placed {
    pub col_idx: IVec,
    /// Where the column's corner is from the corner of the decor cell
    /// drawing it, in units: zero for a z16 cell, its place among the
    /// columns under it for a coarser one.
    pub ox: f64,
    pub oy: f64,
    /// Whether a placed instance keeps the entry it came from, for the
    /// eraser's hit. A coarser cell draws several columns' decor and names
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
    let [x, y] = (1..=level).rev().fold([x, y], |[x, y], l| {
        let slot = derive_seed_u64(derive_seed_u64(derive_seed_u64(DECOR_MASTER_SEED, tag), gather(x, y, 0)), l as u64);
        child_of([x, y], slot as usize)
    });
    (x, y)
}

/// What a kind's scatter draws at a cell of a level: the z16 cell it stands
/// for (`representative`), and that cell's own draws.
pub struct Draw {
    pub gx: i64,
    pub gy: i64,
    pub variant: u32,
    pub rotation: f64,
    pub scale: f64,
}

/// `kind`'s candidate at the cell `at` of `level`, `page_at` its place in
/// the page whose `cells` it reads: none where the kind may not stand on
/// what is drawn there, or its density leaves it out.
pub fn draw(kind: &Kind, cells: &[GroundCell], level: u32, at: [i64; 2], page_at: [u32; 2]) -> Option<Draw> {
    let [cell_x, cell_y] = page_at;
    if !kind.rules.allows(cells[GROUND.cell_index(cell_x, cell_y) as usize]) {
        return None;
    }
    let (gx, gy) = representative(kind.tag, level, at[0], at[1]);
    // The z16 cell's own stream: its page's seed, the kind's, and its place
    // in its page.
    let z16 = GROUND.cell_of(0, &GROUND.scale(0).base_of([gx, gy, 0]));
    let [px, py] = z16.in_page().map(u64::from);
    let kind_seed = derive_seed_u64(derive_seed_u64(DECOR_MASTER_SEED, z16.page().page), kind.tag);
    let mut rng = Rng::new(derive_seed_u64(kind_seed, (px << 16) | py));
    if rng.unit() >= kind.density {
        return None;
    }
    let variant = ((rng.unit() * kind.variant_count as f64).floor() as u32).min(kind.variant_count - 1);
    let rotation = if kind.align_transport {
        let flip = rng.unit() < 0.5;
        transport_aligned_rotation(cells, cell_x, cell_y, flip)
    } else {
        rng.unit() * core::f64::consts::PI * 2.0
    };
    let scale = 1.0 + (rng.unit() * 2.0 - 1.0) * SCALE_JITTER;
    Some(Draw { gx, gy, variant, rotation, scale })
}

/// Whether the scatter stands decor of value `value` in the cell `at` lies
/// in, as z16 draws it, with nothing written there: the kind's candidate at
/// that z16 cell, on the ground drawn there. False where the cell's page is
/// not resident, which says nothing of it.
pub fn scatters_at(value: u32, at: &Pvf3) -> bool {
    let Some(kind) = kinds().iter().find(|k| k.decor_value == Some(value)) else { return false };
    let z16 = GROUND.cell_of(0, at);
    let Some(page) = Terrain::pyramid().get(z16.page()) else { return false };
    let Some(ground) = Access::now().at(at, FINEST).exact() else { return false };
    let [ix, iy, iz] = CELL.index_of(at);
    iz == (ground.top / CELL.side()).round() as i64 && draw(kind, &page.ground.cells, 0, [ix, iy], z16.in_page()).is_some()
}

impl<'s> DecorBuild<'s> {
    /// Builds every kind's instances at world step `step` into `scratch`:
    /// the decor placed in `placed`'s columns, `hide` flagged hidden, and
    /// where `decor`, the scatter over `ground`. A cell scatters from its
    /// own level's page, so decoration reaches as far as the ground it
    /// stands on is drawn, a kind as far as it covers the pixels it is drawn
    /// at (`decor_density`). A cell is a whole column: every instance over
    /// it, at any height. None where the heap refused the scratch room for
    /// them, and the cell is not built.
    #[allow(clippy::too_many_arguments)]
    pub fn build(scratch: &'s mut DecorScratch, kinds: &[Kind], ground: Option<&Ground>, placed: &[Placed], hide: Option<EntryAt>, store: &Store, step: u32, decor: bool) -> Option<DecorBuild<'s>> {
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
            // Placed or scattered, a kind is drawn only where it covers the
            // pixels it is drawn at: how far a placed tree is seen is its own
            // size, as a scattered one's is.
            if decor_density(kind.extent, step) == 0.0 {
                continue;
            }
            for column in placed {
                Self::placed(scratch, kind, first, column, hide, store).ok()?;
            }
            if let Some(ground) = ground.filter(|_| decor) {
                Self::scatter(scratch, kind, first, ground, store).ok()?;
            }
        }
        Some(DecorBuild { scratch })
    }

    /// One column's placed instances of `kind`, into its groups from
    /// `first`, in the store's entry order.
    fn placed(scratch: &mut DecorScratch, kind: &Kind, first: usize, column: &Placed, hide: Option<EntryAt>, store: &Store) -> Result<(), TryReserveError> {
        let (Some(value), Some(e)) = (kind.decor_value, store.entries(column.col_idx)) else {
            return Ok(());
        };
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
            // Turned and varied by where it stands, as the ghost that
            // placed it predicted.
            let (variant, rotation) = user_decor_pick(at.corner().i, kind.variant_count);
            let [lx, ly, lz] = at.local();
            let hidden = hide == Some(e.at(k));
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
            )?;
        }
        Ok(())
    }

    /// The scatter of `kind` over `ground`: for each of the decor cell's
    /// cells the kind may stand on, the candidate of the z16 cell it draws from
    /// (`representative`), with the draws that cell makes at z16 — kept at
    /// the kind's density, stood on the surface drawn there, and left out
    /// where a placed object fills the cell as it is drawn.
    ///
    /// Whether a kind may stand on a cell, and which way a kind aligned to
    /// transport faces, are read off the level's own cells, the ground drawn
    /// there: a tree above z16 stands on the grass the view sees, not on a
    /// z16 cell it cannot, so it can come and go with a level as that grass
    /// does.
    fn scatter(scratch: &mut DecorScratch, kind: &Kind, first: usize, ground: &Ground, store: &Store) -> Result<(), TryReserveError> {
        let cells = ground.page.cells;
        let (level, atom) = (GROUND.scale(ground.level), GROUND.scale(0));
        let (lx0, ly0) = (level.index(ground.x0), level.index(ground.y0));
        let has_entries = ground.col_idx.is_some_and(|c| store.entries(c).is_some());
        for dy in 0..CHUNK_CELLS {
            let cell_y = ground.ty0 + dy;
            for dx in 0..CHUNK_CELLS {
                let cell_x = ground.tx0 + dx;
                let Some(Draw { gx, gy, variant, rotation, scale }) = draw(kind, cells, ground.level, [lx0 + dx as i64, ly0 + dy as i64], [cell_x, cell_y]) else {
                    continue;
                };
                let ele = ground.sample.ele[apron_col(dx, dy) as usize] as i32;
                // A scattered instance is what is made there, and an object's
                // leaf in the cell it stands in is what was written over it,
                // as over the ground: a cube puts a cube there, air takes it
                // away, and the leaf gone brings it back.
                if has_entries && ground.col_idx.is_some_and(|c| store.cell(c, dx, dy, ele).is_some()) {
                    continue;
                }
                let filled = |f: &HashMap<[i64; 3], u32>| f.get(&[lx0 + dx as i64, ly0 + dy as i64, level.index(ele as i64)]).is_some_and(|&s| s != AIR);
                if ground.folded.is_some_and(filled) {
                    continue;
                }
                let (x, y) = ((gx as f64 + 0.5) * atom.side() - ground.x0 as f64, (gy as f64 + 0.5) * atom.side() - ground.y0 as f64);
                let z = ele as f64;
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
                )?;
            }
        }
        Ok(())
    }

    /// Each instance with the group it is drawn in.
    pub(crate) fn each(&self) -> impl Iterator<Item = (usize, &Instance)> {
        self.scratch.groups.iter().map(|&g| g as usize).zip(&self.scratch.instances)
    }

    /// The instances the block holds.
    pub fn instances(&self) -> usize {
        self.scratch.instances.len()
    }

    /// The block's size in bytes (instance.rs `GroupLanes`).
    pub fn bytes(&self) -> u32 {
        let s = self.scratch;
        let words: usize = 1 + s.group_count + s.counts[..s.group_count].iter().map(|&n| GroupLanes::words(n as usize)).sum::<usize>();
        word_bytes(words)
    }

    /// Writes the block: each group's lanes at the group's place, each
    /// instance at its rank among its group's, in the order they were made.
    ///
    /// # Safety
    /// `at` must address `bytes()` bytes, 4-aligned, the caller's to write.
    pub unsafe fn write(&self, at: *mut u8) {
        let s = self.scratch;
        let words = at.cast::<u32>();
        let groups = s.group_count;
        *words = groups as u32;
        let mut lanes = [GroupLanes { first: 0, count: 0 }; DECOR_GROUPS];
        let mut k = 1 + groups;
        for (g, lane) in lanes.iter_mut().enumerate().take(groups) {
            let n = s.counts[g] as usize;
            *words.add(1 + g) = n as u32;
            *lane = GroupLanes { first: k, count: n };
            // The flags are bytes padded to a word, and a pad byte is zero.
            core::ptr::write_bytes(at.add(lane.flags() * 4), 0, n.div_ceil(4) * 4);
            k += GroupLanes::words(n);
        }
        let mut rank = [0usize; DECOR_GROUPS];
        for (i, &g) in s.instances.iter().zip(&s.groups) {
            let (lane, r) = (lanes[g as usize], rank[g as usize]);
            for (l, v) in [i.x, i.y, i.z, i.rotation, i.scale].into_iter().enumerate() {
                *words.add(lane.f32_at(l, r)) = v.to_bits();
            }
            *at.add(lane.flags() * 4 + r) = i.flags;
            *words.add(lane.band_at(r)) = i.band;
            *words.add(lane.pos_at(r)) = i.pos;
            rank[g as usize] += 1;
        }
    }
}

#[cfg(test)]
#[path = "build.test.rs"]
mod test;
