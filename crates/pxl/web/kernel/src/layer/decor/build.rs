// pxl-kernel/layer/decor/build.rs

//! A chunk's decor build. Scattered decor is never stored: it is a pure function of terrain
//! and a fixed seed, and a cell holding a placed object filters its
//! candidate out at build time.

use pxl_render::gpu::api::word_bytes;
use pxl_render::mesh::paint::terrain::adapter::{ChunkSample, PageView};
use pxl_render::mesh::voxel_scan::apron_col;
use pxl_spec::bake::constant::{unpack_pos, U_PER_CELL};
use pxl_spec::procedural::{scatter, scatter_at, Candidate, MASTER_SEED};
use pxl_spec::space::lod::target_cell_px;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND, GROUND_Z};
use pxl_spec::space::v3::Aabb;
use xpute_core::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

use std::collections::{HashMap, TryReserveError};

use crate::layer::decor::instance::{DecorScratch, GroupLanes, Instance, DECOR_GROUPS, HIDDEN_FLAG, NO_ENTRY};
use crate::layer::decor::kind::{Kind, KINDS};
use crate::layer::decor::scatter::{transport_aligned_rotation, SCALE_JITTER};
use crate::layer::object::store::{At, EntryAt, Store, AIR};
use crate::world::access::Access;
use crate::world::pyramid::Layer as _;
use pxl_spec::asset::ingested::Asset;
use pxl_spec::bake::constant::CELL;
use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::GroundCell;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::math::scalar::round;

/// A kind is drawn as a model while it covers at least two of a tier's cells
/// on screen (twelve pixels at the reference viewport, by eye, where Google's
/// maps stop drawing buildings in 3D), then as a sprite down to
/// `IMPOSTOR_GONE_CELLS`. No fade: the edge is per instance, so it staggers.
pub fn decor_gone_px() -> f64 {
    target_cell_px() * 2.0
}

/// Under this a kind's sprite is not drawn.
pub fn decor_sprite_px() -> f64 {
    target_cell_px() * crate::param::IMPOSTOR_GONE_CELLS
}

fn decor_shown(extent_u: f64, lod_step: u32) -> bool {
    (extent_u / U_PER_CELL) * target_cell_px() / lod_step as f64 >= decor_sprite_px()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorVariantAsset {
    pub model: Asset,
    /// A kind's variants may come from different packs with different colormaps.
    pub skin: Asset,
}

/// NaN is kept rather than dropped.
pub fn largest(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, |a: f64, b| if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) })
}

pub fn variant_extent(v: &DecorVariantAsset) -> f64 {
    largest(v.model.ingested().extent.unwrap_or([0.0; 3]).into_iter())
}

/// Seeded only by the placed object's position, its one stable identity, so
/// the hover ghost predicts exactly the variant and heading a placement gets.
pub fn user_decor_pick(i: IVec, variant_count: u32) -> (u32, f64) {
    let mut stream = SplitMix64::new(derive_seed_u64(MASTER_SEED, i));
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

pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
}

pub struct Ground<'a> {
    pub page: &'a PageView<'a>,
    pub sample: &'a ChunkSample,
    pub z: u32,
    pub x0: i64,
    pub y0: i64,
    pub tx0: u32,
    pub ty0: u32,
    /// At z16, the column whose placed objects keep a candidate off their cell.
    /// Above z16 the objects are read folded instead.
    pub col_idx: Option<IVec>,
    pub folded: Option<&'a HashMap<[i64; 3], u32>>,
}

pub struct Placed {
    pub col_idx: IVec,
    /// The column's corner relative to the chunk drawing it, in units.
    pub ox: f64,
    pub oy: f64,
    /// A coarser cell draws several columns' decor and names none of their
    /// entries, so only a z16 cell's placed instances are hittable.
    pub hittable: bool,
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

/// What `candidate` draws in the page's cell `page_at`: none off the kind's
/// ground or under a line.
pub fn draw(kind: &Kind, candidate: Candidate, cells: &[GroundCell], lines: &Decks, page_at: [u32; 2]) -> Option<Draw> {
    let [cell_x, cell_y] = page_at;
    if !kind.rules.allows(cells[GROUND.cell_index(cell_x, cell_y) as usize]) {
        return None;
    }
    let mut rng = Rng::new(candidate.seed);
    let variant = ((rng.unit() * kind.variant_count() as f64).floor() as u32).min(kind.variant_count() - 1);
    let rotation = if kind.align_transport() {
        let flip = rng.unit() < 0.5;
        transport_aligned_rotation(cells, cell_x, cell_y, flip)
    } else {
        rng.unit() * core::f64::consts::PI * 2.0
    };
    let scale = 1.0 + (rng.unit() * 2.0 - 1.0) * SCALE_JITTER;
    if lines.over_point(None, [f64::from(cell_x) + 0.5, f64::from(cell_y) + 0.5], 1.0).next().is_some() {
        return None;
    }
    let [gx, gy] = candidate.cell;
    Some(Draw { gx, gy, variant, rotation, scale })
}

/// What the z16 scatter stands of `value` in the cell `at` lies in, with
/// nothing written there. None where the page is not resident.
pub fn scatters_at(value: u32, at: &Pvf3) -> Option<Draw> {
    let kind = KINDS.iter().find(|k| k.decor_value() == Some(value))?;
    let z16 = GROUND.cell_of(GROUND_Z, at);
    let page = Terrain::pyramid().paging().get(z16.page())?;
    let ground = Access::now().at(at, GROUND_Z).exact()?;
    let [ix, iy, iz] = CELL.index_of(at);
    let candidate = scatter_at(kind.seed.tag, kind.seed.density, [ix, iy])?;
    (ground.top().map(|top| round(top / CELL.side()) as i64) == Some(iz)).then(|| draw(kind, candidate, &page.ground.cells, &page.decks, z16.in_page()))?
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
            if kind.variant_count() == 0 {
                continue;
            }
            // Past what a block holds, the kinds after have none.
            let first = scratch.group_count;
            if first + kind.variant_count() as usize > DECOR_GROUPS {
                break;
            }
            scratch.group_count += kind.variant_count() as usize;
            if !decor_shown(kind.extent(), step) {
                continue;
            }
            for column in placed {
                Self::placed(scratch, kind, first, column, hide.and_then(DecorHide::placed), store, chunk).ok()?;
            }
            if let Some(ground) = ground.filter(|_| decor) {
                Self::scatter(scratch, kind, first, ground, hide, store, chunk).ok()?;
            }
        }
        Some(DecorBuild { scratch })
    }

    fn placed(scratch: &mut DecorScratch, kind: &Kind, first: usize, column: &Placed, hide: Option<EntryAt>, store: &Store, chunk: Aabb) -> Result<(), TryReserveError> {
        let (Some(value), Some(e)) = (kind.decor_value(), store.entries(column.col_idx)) else {
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
            let (variant, rotation) = user_decor_pick(at.corner().i, kind.variant_count());
            let [lx, ly, lz] = at.local();
            if !(chunk.min[2]..chunk.max[2]).contains(&lz) {
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
            )?;
        }
        Ok(())
    }

    /// Whether a kind may stand on a cell, and its transport alignment, are read
    /// off the zoom's own cells: a tree coarser than z16 stands on the grass the view
    /// sees, not on z16 cells it cannot.
    fn scatter(scratch: &mut DecorScratch, kind: &Kind, first: usize, ground: &Ground, hide: Option<DecorHide>, store: &Store, chunk: Aabb) -> Result<(), TryReserveError> {
        let (cells, lines) = (ground.page.cells, ground.page.decks);
        let (own, atom) = (GROUND.side_at(ground.z), GROUND.side_at(GROUND_Z));
        let (lx0, ly0) = (own.index(ground.x0), own.index(ground.y0));
        let per = i64::from(atom.count_in(own));
        let min = [atom.index(ground.x0), atom.index(ground.y0)];
        let span = i64::from(CHUNK_CELLS) * per;
        let has_entries = ground.col_idx.is_some_and(|c| store.entries(c).is_some());
        scatter(kind.seed.tag, kind.seed.density, ground.z, [min, [min[0] + span, min[1] + span]], &mut scratch.candidates)?;
        for i in 0..scratch.candidates.len() {
            let candidate = scratch.candidates[i];
            let (dx, dy) = ((candidate.cell[0] - min[0]).div_euclid(per) as u32, (candidate.cell[1] - min[1]).div_euclid(per) as u32);
            let Some(Draw { gx, gy, variant, rotation, scale }) = draw(kind, candidate, cells, lines, [ground.tx0 + dx, ground.ty0 + dy]) else {
                continue;
            };
            let ele = ground.sample.ele[apron_col(dx, dy) as usize] as i32;
            let at = [(gx as f64 + 0.5) * atom.side(), (gy as f64 + 0.5) * atom.side(), f64::from(ele)];
            // In the chunk holding the cell it stands on.
            if !((0..2).all(|k| (chunk.min[k]..chunk.max[k]).contains(&at[k])) && chunk.min[2] < at[2] && at[2] <= chunk.max[2]) {
                continue;
            }
            // A leaf in the cell overrides the scatter: a cube or air hides it, the leaf gone brings it back.
            if has_entries && ground.col_idx.is_some_and(|c| store.cell(c, dx, dy, ele).is_some()) {
                continue;
            }
            let filled = |f: &HashMap<[i64; 3], u32>| f.get(&[lx0 + dx as i64, ly0 + dy as i64, own.index(ele as i64)]).is_some_and(|&s| s != AIR);
            if ground.folded.is_some_and(filled) {
                continue;
            }
            let [x, y, z] = [at[0] - chunk.min[0], at[1] - chunk.min[1], at[2]];
            let hidden = ground.z == GROUND_Z && hide == Some(DecorHide::Scattered([gx, gy, i64::from(ele)]));
            scratch.push(
                first + variant as usize,
                Instance {
                    x: x as f32,
                    y: y as f32,
                    z: z as f32,
                    rotation: rotation as f32,
                    scale: scale as f32,
                    flags: if hidden { HIDDEN_FLAG } else { 0 },
                    band: NO_ENTRY,
                    pos: 0,
                },
            )?;
        }
        Ok(())
    }

    /// The lowest and highest instance; `[0, 0]` with none.
    pub fn heights(&self) -> [f32; 2] {
        let z = self.scratch.instances.iter().map(|i| i.z);
        match z.clone().reduce(f32::min) {
            Some(lo) => [lo, z.fold(lo, f32::max)],
            None => [0.0, 0.0],
        }
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
