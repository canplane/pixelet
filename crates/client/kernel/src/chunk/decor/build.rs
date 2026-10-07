// pxl-kernel/chunk/decor/build.rs

//! A chunk's decor build. Scattered decor is never stored: it is a pure function of terrain
//! and a fixed seed, and a cell holding a placed object filters its
//! candidate out at build time.

use crate::chunk::Chunk as PyramidChunk;
use crate::layer::Query;
use pxl_render::gpu::api::word_bytes;
use pxl_render::mesh::paint::terrain::sample::{ChunkSample, TileView};
use pxl_render::mesh::run::apron_col;
use pxl_spec::procedural::{scatter, scatter_at};
use pxl_spec::space::camera::grid::target_cell_px;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use std::collections::{HashMap, TryReserveError};

use crate::chunk::decor::instance::{DecorScratch, GroupLanes, Instance, DECOR_GROUPS, HIDDEN_FLAG, NO_ENTRY};
use crate::layer::object::meta::user_decor_pick;
use crate::layer::terrain::scatter::{draw, Draw};
use pxl_spec::cell::decor::{DecorKind, KINDS};
use crate::layer::object::store::{EntryAt, Store, AIR};
use crate::layer::Layer as _;
use crate::query::World;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::zoom::{count, index, index_of, units, Zoom};
use xpute_core::math::scalar::round;

/// A kind is drawn as a model down to `DECOR_MODEL_PX`, then as a sprite
/// down to `DECOR_SPRITE_PX`. No fade: the edge is per instance, so it staggers.
pub fn decor_gone_px() -> f64 {
    crate::param::DECOR_MODEL_PX
}

pub fn decor_sprite_px() -> f64 {
    crate::param::DECOR_SPRITE_PX
}

/// Whether a kind `extent_u` wide is a sprite's worth anywhere a chunk of
/// `cell_zoom` is drawn: its cell is `target_cell_px` where it begins.
fn decor_shown(extent_u: f64, cell_zoom: Zoom) -> bool {
    extent_u / units(cell_zoom) * target_cell_px() >= decor_sprite_px()
}

pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
}

pub struct Ground<'a> {
    pub tile: &'a TileView<'a>,
    pub sample: &'a ChunkSample,
    pub zoom: Zoom,
    pub x0: i64,
    pub y0: i64,
    /// The first tile cell read, in the tile, and how many a side.
    pub tx0: u32,
    pub ty0: u32,
    pub side: u32,
    /// What the objects fold to over cells coarser than a cell; none where
    /// the cells are a cell's, and a leaf written in one keeps a candidate off it.
    pub folded: Option<&'a HashMap<[i64; 3], u32>>,
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

/// What the leaf scatter stands of `value` in the cell `at` lies in, with
/// nothing written there. None where the tile is not resident.
pub fn scatters_at(value: u32, at: &Pvf3) -> Option<Draw> {
    let kind = KINDS.iter().find(|k| u32::from(k.value()) == value)?;
    let leaf = GROUND.cell_of(LEAF_ZOOM, at);
    // The leaf tile's own cells, as the terrain answers them: the leaf scatter reads no coarser.
    let (zoom, [x, y]) = leaf.tile().node();
    let tile = Terrain::residency().read((zoom, [x, y, 0]), GROUND.grid_zoom(zoom)).exact()?.view().tile;
    let ground = World::now().at(at, LEAF_ZOOM).exact()?;
    let [ix, iy, iz] = index_of(GROUND.cell(), at);
    let candidate = scatter_at(kind.seed, kind.density, [ix, iy])?;
    (ground.top().map(|top| round(top / units(GROUND.cell())) as i64) == Some(iz)).then(|| draw(kind, candidate, tile.cells, tile.decks, leaf.in_tile()))?
}

impl<'s> DecorBuild<'s> {
    /// None where the heap refused scratch room, and the cell is not built.
    #[allow(clippy::too_many_arguments)]
    /// Only what stands in `chunk`, the region of the chunk drawing it; the
    /// ground may be a coarser chunk's over it.
    pub fn build(
        scratch: &'s mut DecorScratch,
        kinds: &[DecorKind],
        ground: Option<&Ground>,
        hittable: bool,
        hide: Option<DecorHide>,
        store: &Store,
        decor: bool,
        at: PyramidChunk,
    ) -> Option<DecorBuild<'s>> {
        let (cell_zoom, chunk) = (at.cell_zoom(), at.region());
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
            if !decor_shown(kind.extent(), cell_zoom) {
                continue;
            }
            Self::placed(scratch, kind, first, hittable, hide.and_then(DecorHide::placed), store, at).ok()?;
            if let Some(ground) = ground.filter(|_| decor) {
                Self::scatter(scratch, kind, first, ground, hide, store, chunk).ok()?;
            }
        }
        Some(DecorBuild { scratch })
    }

    /// Only what stands in `chunk`: a placed decor stands at its cube's bottom middle.
    fn placed(scratch: &mut DecorScratch, kind: &DecorKind, first: usize, hittable: bool, hide: Option<EntryAt>, store: &Store, at: PyramidChunk) -> Result<(), TryReserveError> {
        let chunk = at.region();
        let value = u32::from(kind.value());
        let Some(objects) = store.read(at.node(), at.cell_zoom()).value() else { return Ok(()) };
        let mut refused = Ok(());
        objects.entries(&mut |e| {
            let c = e.cube();
            let (x, y) = ((c.min[0] + c.max[0]) / 2.0, (c.min[1] + c.max[1]) / 2.0);
            let stands = (chunk.min[0]..chunk.max[0]).contains(&x) && (chunk.min[1]..chunk.max[1]).contains(&y) && (chunk.min[2]..chunk.max[2]).contains(&c.min[2]);
            if e.state != value || !stands || refused.is_err() {
                return;
            }
            // Turned and varied by position, as the placing ghost predicted.
            let (variant, rotation) = user_decor_pick(e.corner().i, kind.variant_count());
            let hidden = hide == Some(e.id());
            refused = scratch.push(
                first + variant as usize,
                Instance {
                    x: (x - chunk.min[0]) as f32,
                    y: (y - chunk.min[1]) as f32,
                    z: c.min[2] as f32,
                    rotation: rotation as f32,
                    scale: 1.0,
                    flags: if hidden { HIDDEN_FLAG } else { 0 },
                    entry: if hittable { u32::from(e.zoom) } else { NO_ENTRY },
                },
            );
        });
        refused
    }

    /// Whether a kind may stand on a cell, and its transport alignment, are read
    /// off the zoom's own cells: a tree coarser than a leaf's stands on the grass the view
    /// sees, not on leaf cells it cannot.
    fn scatter(scratch: &mut DecorScratch, kind: &DecorKind, first: usize, ground: &Ground, hide: Option<DecorHide>, store: &Store, chunk: Aabb) -> Result<(), TryReserveError> {
        let (cells, lines) = (ground.tile.cells, ground.tile.decks);
        let (own, atom) = (GROUND.cell_zoom(ground.zoom), GROUND.cell_zoom(LEAF_ZOOM));
        let (lx0, ly0) = (index(own, ground.x0), index(own, ground.y0));
        let per = i64::from(count(atom, own));
        let min = [index(atom, ground.x0), index(atom, ground.y0)];
        let span = i64::from(ground.side) * per;
        scatter(kind.seed, kind.density, ground.zoom, [min, [min[0] + span, min[1] + span]], &mut scratch.candidates)?;
        for i in 0..scratch.candidates.len() {
            let candidate = scratch.candidates[i];
            let (dx, dy) = ((candidate.cell[0] - min[0]).div_euclid(per) as u32, (candidate.cell[1] - min[1]).div_euclid(per) as u32);
            let Some(Draw { gx, gy, variant, rotation, scale }) = draw(kind, candidate, cells, lines, [ground.tx0 + dx, ground.ty0 + dy]) else {
                continue;
            };
            let ele = ground.sample.ele[apron_col(ground.sample.span, dx, dy) as usize] as i32;
            let at = [(gx as f64 + 0.5) * units(atom), (gy as f64 + 0.5) * units(atom), f64::from(ele)];
            // In the chunk holding the cell it stands on.
            if !((0..2).all(|k| (chunk.min[k]..chunk.max[k]).contains(&at[k])) && chunk.min[2] < at[2] && at[2] <= chunk.max[2]) {
                continue;
            }
            // A leaf in the cell overrides the scatter: a cube or air hides it, the leaf gone brings it back.
            // Folded, only a fill does.
            let hides = match ground.folded {
                None => store.read((GROUND.cell(), [gx, gy, i64::from(ele)]), GROUND.cell()).value().is_some_and(|c| c.holds()),
                Some(f) => f.get(&[lx0 + dx as i64, ly0 + dy as i64, index(own, ele as i64)]).is_some_and(|&s| s != AIR),
            };
            if hides {
                continue;
            }
            let [x, y, z] = [at[0] - chunk.min[0], at[1] - chunk.min[1], at[2]];
            let hidden = hide == Some(DecorHide::Scattered([gx, gy, i64::from(ele)]));
            scratch.push(
                first + variant as usize,
                Instance {
                    x: x as f32,
                    y: y as f32,
                    z: z as f32,
                    rotation: rotation as f32,
                    scale: scale as f32,
                    flags: if hidden { HIDDEN_FLAG } else { 0 },
                    entry: NO_ENTRY,
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
            *words.add(lane.entry_at(r)) = i.entry;
            rank[g as usize] += 1;
        }
    }
}
