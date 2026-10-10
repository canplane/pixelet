// pxl-kernel/chunk/decor/build.rs

//! A chunk's decor build. Scattered decor is never stored: it is a pure function of terrain
//! and a fixed seed, and a cell holding a placed object filters its
//! candidate out at build time.

use crate::chunk::Chunk as PyramidChunk;
use crate::layer::Query;
use pxl_render::gpu::api::word_bytes;
use pxl_render::mesh::paint::terrain::sample::TileSource;
use pxl_spec::space::camera::grid::target_cell_px;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::layer::lattice::GROUND;

use std::collections::{HashMap, TryReserveError};

use crate::chunk::decor::instance::{DecorScratch, GroupLanes, DECOR_GROUPS, HIDDEN_FLAG};
use crate::layer::instance::{Meta, Source};
use crate::layer::object::meta::placed;
use crate::layer::object::store::{EntryAt, Store};
use crate::layer::terrain::scatter::grow;
use crate::query::{covers, Over};
use pxl_spec::cell::decor::DecorKind;
use pxl_spec::space::coord::zoom::{index, units, Zoom};

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
    /// The chunk's corner, which its lanes are laid about.
    corner: [f64; 2],
    hide: Option<DecorHide>,
}

pub struct Ground<'a> {
    /// The tile the chunk is drawn from, as the terrain answers it.
    pub source: TileSource<'a>,
    pub zoom: Zoom,
    pub x0: i64,
    pub y0: i64,
    /// The first tile cell read, in the tile, and how many a side.
    pub tx0: u32,
    pub ty0: u32,
    pub side: u32,
    /// What the objects fold to over cells coarser than a cell; none where
    /// the cells are a cell's.
    pub folded: Option<&'a HashMap<[i64; 3], u32>>,
}

/// What the eraser is over, hidden in its block while it is.
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

impl<'s> DecorBuild<'s> {
    /// None where the heap refused scratch room, and the cell is not built.
    #[allow(clippy::too_many_arguments)]
    /// Only what stands in `chunk`, the region of the chunk drawing it; the
    /// ground may be a coarser chunk's over it.
    pub fn build(scratch: &'s mut DecorScratch, kinds: &[DecorKind], ground: Option<&Ground>, hide: Option<DecorHide>, store: &Store, decor: bool, at: PyramidChunk) -> Option<DecorBuild<'s>> {
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
            Self::placed(scratch, kind, first, store, at).ok()?;
            if let Some(ground) = ground.filter(|_| decor) {
                Self::grown(scratch, kind, first, ground, store, chunk).ok()?;
            }
        }
        Some(DecorBuild {
            scratch,
            corner: [chunk.min[0], chunk.min[1]],
            hide,
        })
    }

    /// Only what stands in `chunk`.
    fn placed(scratch: &mut DecorScratch, kind: &DecorKind, first: usize, store: &Store, at: PyramidChunk) -> Result<(), TryReserveError> {
        let chunk = at.region();
        let Some(objects) = store.read(at.node(), at.cell_zoom()).value() else { return Ok(()) };
        let mut refused = Ok(());
        objects.entries(&mut |e| {
            let Some(i) = placed(e).filter(|i| i.id == kind.value()) else { return };
            let stands = (0..3).all(|k| (chunk.min[k]..chunk.max[k]).contains(&i.at[k]));
            let Meta::Decor(m) = i.meta;
            if stands && refused.is_ok() {
                refused = scratch.push(first + m.variant as usize, i);
            }
        });
        refused
    }

    /// What the ground grows here, less what the objects over it cover.
    fn grown(scratch: &mut DecorScratch, kind: &DecorKind, first: usize, ground: &Ground, store: &Store, chunk: Aabb) -> Result<(), TryReserveError> {
        let own = GROUND.cell_zoom(ground.zoom);
        let mut candidates = core::mem::take(&mut scratch.candidates);
        let grew = grow(
            kind,
            &ground.source,
            ground.zoom,
            [ground.x0, ground.y0],
            [ground.tx0, ground.ty0],
            ground.side,
            chunk,
            &mut candidates,
            |i| {
                let (Source::Ground(cell), Meta::Decor(m)) = (i.source, i.meta) else { return Ok(()) };
                let over = match ground.folded {
                    None => Over::Leaf {
                        held: store.read((GROUND.cell(), cell), GROUND.cell()).value().is_some_and(|c| c.holds()),
                    },
                    Some(f) => Over::Folded(f.get(&cell.map(|c| index(own, c))).copied()),
                };
                if covers(over) {
                    return Ok(());
                }
                scratch.push(first + m.variant as usize, i)
            },
        );
        scratch.candidates = candidates;
        grew
    }

    /// The lowest and highest instance; `[0, 0]` with none.
    pub fn heights(&self) -> [f32; 2] {
        let z = self.scratch.instances.iter().map(|i| i.at[2] as f32);
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

    /// Each instance at its rank within its group, in creation order, about
    /// the chunk's corner.
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
            let Meta::Decor(m) = i.meta;
            let local = [i.at[0] - self.corner[0], i.at[1] - self.corner[1], i.at[2], m.rotation, m.scale];
            for (l, v) in local.into_iter().enumerate() {
                *words.add(lane.f32_at(l, r)) = (v as f32).to_bits();
            }
            let hidden = match (self.hide, i.source) {
                (Some(DecorHide::Placed(h)), Source::Object(e)) => h == e,
                (Some(DecorHide::Scattered(h)), Source::Ground(c)) => h == c,
                _ => false,
            };
            *at.add(lane.flags() * 4 + r) = if hidden { HIDDEN_FLAG } else { 0 };
            rank[g as usize] += 1;
        }
    }
}
