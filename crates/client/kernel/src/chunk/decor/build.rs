// pxl-kernel/chunk/decor/build.rs

//! A chunk's decor build. Scattered decor is never stored: it is a pure function of terrain
//! and a fixed seed, and a cell holding a placed object filters its
//! candidate out at build time.

use crate::chunk::Chunk as PyramidChunk;
use crate::layer::Query;
use pxl_render::gpu::api::word_bytes;
use pxl_spec::space::camera::grid::{px_per_unit, split_distance, REFERENCE_FOV_DEG, REFERENCE_VIEWPORT_H};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::hierarchy::tree::Node;

use std::collections::TryReserveError;

use crate::chunk::decor::instance::{DecorScratch, GroupLanes, DECOR_GROUPS, HIDDEN_FLAG};
use crate::decor::strike::reach;
use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::meta::placed;
use crate::layer::object::store::{EntryAt, Store};
use crate::query::World;
use crate::query::{grow, ring, Surfaces};
use pxl_spec::block::decor::Decor;
use pxl_spec::space::coord::zoom::Zoom;

/// A decor is drawn as a model down to `DECOR_MODEL_PX`, then as a sprite
/// down to `DECOR_SPRITE_PX`. No fade: the edge is per instance, so it staggers.
pub fn decor_gone_px() -> f64 {
    crate::param::DECOR_MODEL_PX
}

pub fn decor_sprite_px() -> f64 {
    crate::param::DECOR_SPRITE_PX
}

/// Whether a decor `extent_u` wide is a sprite's worth anywhere a chunk of
/// `cell_zoom` is drawn: as it spans where that zoom begins, at the
/// reference view.
fn decor_shown(extent_u: f64, cell_zoom: Zoom) -> bool {
    extent_u / split_distance(cell_zoom) * px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) >= decor_sprite_px()
}

pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
    /// The chunk's corner, which its lanes are laid about.
    corner: [f64; 2],
    hide: Option<DecorHide>,
}

/// Where the chunk's ground is read: the node over it whose grid cells at
/// `cell` read the tile it is drawn from, and that node's corner.
pub struct Ground {
    pub node: Node<2>,
    pub cell: Zoom,
    pub origin: [i64; 2],
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
    pub fn build(scratch: &'s mut DecorScratch, decors: &[Decor], ground: Option<&Ground>, hide: Option<DecorHide>, store: &Store, grows: bool, at: PyramidChunk) -> Option<DecorBuild<'s>> {
        let (cell_zoom, chunk) = (at.cell_zoom(), at.region());
        scratch.clear();
        // The ground every decor grows on, read once for all of them, and
        // what placed decor claims about the chunk, as far as a box standing
        // in it reaches past it.
        let world = World::now();
        if grows && ground.is_some() {
            let r = reach();
            let about = Aabb {
                min: chunk.min.map(|v| v - r),
                max: chunk.max.map(|v| v + r),
            };
            world.placed_claims(&about, &mut scratch.placed).ok()?;
        }
        let mut each = |mut face: Option<(&mut Surfaces, &Ground)>| -> Result<(), TryReserveError> {
            for decor in decors {
                if decor.variant_count() == 0 {
                    continue;
                }
                // Past what a block holds, the decors after have none.
                let first = scratch.group_count;
                if first + decor.variant_count() as usize > DECOR_GROUPS {
                    break;
                }
                scratch.group_count += decor.variant_count() as usize;
                if !decor_shown(decor.extent(), cell_zoom) {
                    continue;
                }
                Self::placed(scratch, decor, first, store, at)?;
                if let Some((face, ground)) = face.as_mut() {
                    Self::grown(scratch, &world, face, decor, first, ground, chunk)?;
                }
            }
            Ok(())
        };
        let built = match ground.filter(|_| grows) {
            Some(g) => world.surfaces(g.node, g.cell, ring(decors), |face| each(Some((face, g)))).value(),
            None => None,
        };
        built.unwrap_or_else(|| each(None)).ok()?;
        Some(DecorBuild {
            scratch,
            corner: [chunk.min[0], chunk.min[1]],
            hide,
        })
    }

    /// Only what stands in `chunk`.
    fn placed(scratch: &mut DecorScratch, decor: &Decor, first: usize, store: &Store, at: PyramidChunk) -> Result<(), TryReserveError> {
        let chunk = at.region();
        let Some(objects) = store.read(at.node(), at.cell_zoom()).value() else { return Ok(()) };
        let mut refused = Ok(());
        objects.entries(&mut |e| {
            let Some(i) = placed(e).filter(|i| i.id == decor.value()) else { return };
            // In the chunk its box's corner is in.
            let corner = e.cube().min;
            let stands = (0..3).all(|k| (chunk.min[k]..chunk.max[k]).contains(&corner[k]));
            let Meta::Decor(m) = i.meta;
            if stands && refused.is_ok() {
                refused = scratch.push(first + m.variant as usize, i);
            }
        });
        refused
    }

    /// What the ground grows here, where the chunks draw its footing.
    fn grown(scratch: &mut DecorScratch, world: &World, face: &mut Surfaces, decor: &Decor, first: usize, ground: &Ground, chunk: Aabb) -> Result<(), TryReserveError> {
        let (mut candidates, placed) = (core::mem::take(&mut scratch.candidates), core::mem::take(&mut scratch.placed));
        let grew = grow(world, face, decor, ground.node, ground.cell, ground.origin, chunk, &placed, &mut candidates, |i: Instance| {
            let Meta::Decor(m) = i.meta;
            scratch.push(first + m.variant as usize, i)
        });
        (scratch.candidates, scratch.placed) = (candidates, placed);
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
            let local = [i.at[0] - self.corner[0], i.at[1] - self.corner[1], i.at[2], m.spin(), m.scale];
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
