// pxl-kernel/layer/decor/build.rs

//! The decor layer: the kinds taught to the scheduler, and a decor cell's
//! build. Scattered decor is never stored: it is a pure function of terrain
//! and a fixed seed, and a cell holding a placed object filters its
//! candidate out at build time.

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

use std::collections::HashMap;

use crate::layer::decor::instance::{DecorScratch, GroupLanes, Instance, DECOR_GROUPS, HIDDEN_FLAG, NO_ENTRY};
use crate::layer::decor::kind::DecorKindDef;
use crate::layer::decor::scatter::{transport_aligned_rotation, SCALE_JITTER};
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};
use crate::layer::object::store::{At, EntryAt, Store};
use pxl_spec::asset::ingested::Asset;

/// A kind is drawn as a model while it covers at least two of a tier's cells
/// on screen (twelve pixels at the reference viewport, by eye, where Google's
/// maps stop drawing buildings in 3D), then as a sprite down to
/// `IMPOSTOR_GONE_CELLS`. No fade: the edge is per instance, so it staggers.
pub fn decor_gone_px() -> f64 {
    *TARGET_CELL_PX * 2.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorTier {
    Model,
    /// A quad an instance: too small for a model's triangles, large enough to be seen.
    Impostor,
    Gone,
}

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

pub fn decor_density(extent_u: f64, lod_step: LodStep) -> f64 {
    let projected_px = (extent_u / U_PER_CELL) * *TARGET_CELL_PX / lod_step as f64;
    if projected_px < decor_gone_px() {
        0.0
    } else {
        1.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorVariantAsset {
    pub model: Asset,
    /// A kind's variants may come from different packs with different colormaps.
    pub skin: Asset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecorKindBundle {
    pub seed: DecorKindSeed,
    pub rules: DecorRules,
    pub variants: Vec<DecorVariantAsset>,
    /// None for a kind with no user-placeable equivalent.
    pub decor_id: Option<DecorId>,
    /// The largest dimension over every variant's model, in world units.
    pub extent: f64,
}

/// NaN is kept rather than dropped.
fn largest(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, |a: f64, b| if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) })
}

pub fn variant_extent(v: &DecorVariantAsset) -> f64 {
    largest(v.model.ingested().extent.unwrap_or([0.0; 3]).into_iter())
}

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

static KINDS: Global<Vec<Kind>> = Global::new();

/// In taught order, which is the order of the groups in every decor block.
pub fn kinds() -> &'static [Kind] {
    KINDS.get(Vec::new)
}

/// One group per variant, kinds in bundle order.
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

/// Seeded only by the placed object's position, its one stable identity, so
/// the hover ghost predicts exactly the variant and heading a placement gets.
pub fn user_decor_pick(i: IVec, variant_count: u32) -> (u32, f64) {
    let mut stream = SplitMix64::new(derive_seed_u64(DECOR_MASTER_SEED, i));
    let mut rng = || stream.next_u32();
    let variant = ((u32_to_unit(rng()) * variant_count as f64).floor() as u32).min(variant_count - 1);
    let rotation = u32_to_unit(rng()) * core::f64::consts::PI * 2.0;
    (variant, rotation)
}

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
    pub extent: f64,
    pub decor_value: Option<u32>,
    pub rules: DecorRules,
}

pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
}

pub struct Ground<'a> {
    pub page: &'a PageView<'a>,
    pub sample: &'a ChunkSample,
    pub level: u32,
    pub x0: i64,
    pub y0: i64,
    pub tx0: u32,
    pub ty0: u32,
    /// At z16, the column whose placed objects keep a candidate off their cell.
    /// Above z16 the objects are read folded instead.
    pub col_idx: Option<IVec>,
    /// Above z16, the chunk's objects folded to its level's cell, as its
    /// runs draw them: a candidate is kept off a cell they fill.
    pub folded: Option<&'a HashMap<[i64; 3], u32>>,
}

pub struct Placed {
    pub col_idx: IVec,
    /// The column's corner relative to the decor cell drawing it, in units.
    pub ox: f64,
    pub oy: f64,
    /// A coarser cell draws several columns' decor and names none of their
    /// entries, so only a z16 cell's placed instances are hittable.
    pub hittable: bool,
}

/// The z16 cell a level cell's candidate is drawn from: one of four children
/// per level, keyed to the kind and node alone, so zooming keeps the
/// instances it had and adds or drops a quarter per level.
fn representative(tag: u64, level: u32, x: i64, y: i64) -> (i64, i64) {
    let [x, y] = (1..=level).rev().fold([x, y], |[x, y], l| {
        let slot = derive_seed_u64(derive_seed_u64(derive_seed_u64(DECOR_MASTER_SEED, tag), gather(x, y, 0)), l as u64);
        child_of([x, y], slot as usize)
    });
    (x, y)
}

/// Hidden rather than left out, so the hit that put the eraser there still
/// meets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorHide {
    Placed(EntryAt),
    Scattered([i64; 3]),
}

impl DecorHide {
    pub fn placed(self) -> Option<EntryAt> {
        match self {
            DecorHide::Placed(at) => Some(at),
            DecorHide::Scattered(_) => None,
        }
    }
}

pub struct Draw {
    pub gx: i64,
    pub gy: i64,
    pub variant: u32,
    pub rotation: f64,
    pub scale: f64,
}

/// `kind`'s candidate at cell `at` of `level`. None where the shown ground
/// is not the kind's, where any deck, ramp or bore line is in the cell, or
/// where the density leaves it out.
pub fn draw(kind: &Kind, cells: &[GroundCell], lines: &Decks, level: u32, at: [i64; 2], page_at: [u32; 2]) -> Option<Draw> {
    let [cell_x, cell_y] = page_at;
    if !kind.rules.allows(cells[GROUND.cell_index(cell_x, cell_y) as usize]) {
        return None;
    }
    let (gx, gy) = representative(kind.tag, level, at[0], at[1]);
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
    // After the draws, so a page's lines are walked only for cells a kind would fill.
    if lines.over_point([f64::from(cell_x) + 0.5, f64::from(cell_y) + 0.5]).next().is_some() {
        return None;
    }
    Some(Draw { gx, gy, variant, rotation, scale })
}

/// What the z16 scatter stands of `value` in the cell `at` lies in, with
/// nothing written there. None where the page is not resident.
pub fn scatters_at(value: u32, at: &Pvf3) -> Option<Draw> {
    let kind = kinds().iter().find(|k| k.decor_value == Some(value))?;
    let z16 = GROUND.cell_of(0, at);
    let page = Terrain::pyramid().get(z16.page())?;
    let ground = Access::now().at(at, FINEST).exact()?;
    let [ix, iy, iz] = CELL.index_of(at);
    (iz == (ground.top / CELL.side()).round() as i64).then(|| draw(kind, &page.ground.cells, &page.decks, 0, [ix, iy], z16.in_page()))?
}

impl<'s> DecorBuild<'s> {
    /// None where the heap refused scratch room, and the cell is not built.
    #[allow(clippy::too_many_arguments)]
    /// Only what stands in `chunk`, the region of the chunk drawing it; the
    /// ground may be a coarser chunk's over it.
    pub fn build(
        scratch: &'s mut DecorScratch,
        kinds: &[&Kind],
        ground: Option<&Ground>,
        placed: &[Placed],
        hide: Option<DecorHide>,
        store: &Store,
        step: u32,
        decor: bool,
        chunk: Aabb,
    ) -> Option<DecorBuild<'s>> {
        scratch.clear();
        for kind in kinds {
            if kind.variant_count == 0 {
                continue;
            }
            // Past what a block holds, the kinds after have none.
            let first = scratch.group_count;
            if first + kind.variant_count as usize > DECOR_GROUPS {
                break;
            }
            scratch.group_count += kind.variant_count as usize;
            if decor_density(kind.extent, step) == 0.0 {
                continue;
            }
            for column in placed {
                Self::placed(scratch, kind, first, column, hide, store, heights);
            }
            if let Some(ground) = ground.filter(|_| decor) {
                Self::scatter(scratch, kind, first, ground, store, heights);
            }
        }
        DecorBuild { scratch }
    }

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
            // Turned and varied by position, as the placing ghost predicted.
            let (variant, rotation) = user_decor_pick(at.corner().i, kind.variant_count);
            let [lx, ly, lz] = at.local();
            if lz < z0 || lz >= z1 {
                continue;
            }
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
            );
        }
    }

    /// Whether a kind may stand on a cell, and its transport alignment, are read
    /// off the level's own cells: a tree above z16 stands on the grass the view
    /// sees, not on z16 cells it cannot.
    fn scatter(scratch: &mut DecorScratch, kind: &Kind, first: usize, ground: &Ground, hide: Option<DecorHide>, store: &Store) -> Result<(), TryReserveError> {
        let (cells, lines) = (ground.page.cells, ground.page.decks);
        let (level, atom) = (GROUND.scale(ground.level), GROUND.scale(0));
        let (lx0, ly0) = (level.index(ground.x0), level.index(ground.y0));
        let has_entries = ground.col_idx.is_some_and(|c| store.entries(c).is_some());
        for dy in 0..CHUNK_CELLS {
            let cell_y = ground.ty0 + dy;
            for dx in 0..CHUNK_CELLS {
                let cell_x = ground.tx0 + dx;
                if !kind.rules.allows(cells[GROUND.cell_index(cell_x, cell_y) as usize]) {
                    continue;
                }
                let (gx, gy) = representative(kind.tag, ground.level, lx0 + dx as i64, ly0 + dy as i64);
                // The z16 cell's own stream: its page's seed, the kind's, and
                // its place in its page.
                let z16 = GROUND.cell_of(0, &atom.base_of([gx, gy, 0]));
                let [px, py] = z16.in_page().map(u64::from);
                let kind_seed = derive_seed_u64(derive_seed_u64(DECOR_MASTER_SEED, z16.page().page), kind.tag);
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
                // A leaf in the cell overrides the scatter: a cube or air hides it, the leaf gone brings it back.
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
                );
            }
        }
        Ok(())
    }

    pub fn instances(&self) -> usize {
        self.scratch.instances.len()
    }

    pub fn bytes(&self) -> u32 {
        let s = self.scratch;
        let words: usize = 1 + s.group_count + s.counts[..s.group_count].iter().map(|&n| GroupLanes::words(n as usize)).sum::<usize>();
        word_bytes(words)
    }

    /// Each instance at its rank within its group, in creation order.
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
            core::ptr::write_bytes(at.add(lane.flags() * 4), 0, n.div_ceil(4) * 4);
            k += GroupLanes::words(n);
        }
        let mut rank = [0usize; DECOR_GROUPS];
        for (i, &g) in s.instances[..s.len].iter().zip(&s.groups[..s.len]) {
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
