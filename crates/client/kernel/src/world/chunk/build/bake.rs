// pxl-kernel/world/chunk/build/bake.rs

//! A tier's bake: the mesh written a slot at a time into `.fixed.bake` and handed to the table.

use pxl_spec::space::layer::lattice::GROUND;

use pxl_render::gpu::api::{gpu, room};
use pxl_render::gpu::renderer::device_ready;
use pxl_render::mesh::chunk_mesher::{box_words, Step, COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_spec::bake::constant::EntryAt;
use std::collections::HashMap;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::Block;

use crate::layer::object::store::AIR;
use crate::mem::fixed::{at, Range};
use crate::setting::store::settings_store;

use crate::world::chunk::mesh_slot::{capacity, COARSE_BOXES, FINE_BOXES};
use pxl_spec::space::coord::world::around;
use pxl_spec::space::coord::zoom::{base_of, count, index_of, units, Zoom};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use super::*;

/// `.fixed.bake`: the open bake writes its mesh here one slot's worth at a
/// time, so it is sized to the largest slot.
pub const BAKE_BYTES: u32 = {
    let (fine, coarse) = (FINE_BOXES * FINE_BOX_WORDS, COARSE_BOXES * COARSE_BOX_WORDS);
    4 * if fine > coarse { fine } else { coarse }
};

pub(super) struct BakeJob {
    pub(super) id: u32,
    pub(super) generation: u32,
    pub(super) mesh: Option<Block>,
    pub(super) handed: u32,
}

/// The eraser's preview: the cells at `zoom` its entry covers, cut to air,
/// X in the frame of the cells from `from_x`.
pub(super) fn carve_into(cells: &mut HashMap<[i64; 3], u32>, carve: EntryAt, zoom: Zoom, from_x: i64) {
    let own = carve.zoom();
    let [ix, iy, iz] = index_of(zoom, &base_of(own, carve.at()));
    let ix = from_x + (ix - from_x).rem_euclid(around(zoom));
    let n = if own < zoom { i64::from(count(zoom, own)) } else { 1 };
    for dz in 0..n {
        for dy in 0..n {
            for dx in 0..n {
                cells.insert([ix + dx, iy + dy, iz + dz], AIR);
            }
        }
    }
}

impl Builder {
    pub(super) fn abort_bake(&mut self) {
        if self.bake_job.take().is_some() {
            self.bake.abort();
        }
    }

    pub(super) fn advance_bake(&mut self, id: u32, emit: &mut impl FnMut(BuildEvent) -> bool) -> Progress {
        let holding = matches!(&self.bake_job, Some(j) if j.id == id);
        let c = &self.chunks[&id];
        let runs = self.runs_of(id).or_bug(Errno::ENOTRECOVERABLE);
        let set = Self::run_set(&runs, run_span(self.chunks[&id].at, runs.zoom));
        if !holding {
            if self.bake_job.is_some() {
                return Progress::Blocked;
            }
            let cell_zoom = c.at.cell_zoom();
            // Runs span the whole column; this chunk bakes only its own heights, in its runs' cells.
            let (z0, z1) = c.at.z_range();
            let z = z_unit(runs.zoom);
            let clip = Some(((z0 / z) as i32, (z1 / z) as i32));
            // Textured where the cells drawn are a leaf's or finer, folded or not.
            let (unit, fine) = (units(runs.zoom), cell_zoom >= GROUND.cell());
            if self
                .bake
                .begin(
                    &set,
                    count(runs.zoom, cell_zoom),
                    unit,
                    z,
                    fine,
                    settings_store().textures,
                    (crate::param::BAKE_ROWS_PER_STEP, crate::param::BAKE_BOXES_PER_STEP),
                    clip,
                )
                .is_err()
            {
                self.refuse(id);
                return Progress::Blocked;
            }
            self.bake_job = Some(BakeJob {
                id,
                generation: c.generation,
                mesh: None,
                handed: 0,
            });
        }
        match self.bake.step(&set) {
            Step::Working => Progress::Busy,
            Step::Refused => {
                self.bake_job = None;
                self.refuse(id);
                Progress::Blocked
            }
            Step::NeedMesh(words) => {
                // All slots are held at once or the bake is thrown away: a mesh is never
                // cut, never baked coarser, and never waits while holding the one bake.
                let job = self.bake_job.as_ref().unwrap();
                let cell_zoom = c.at.cell_zoom();
                let (words_a_box, room_a_slot) = (box_words(cell_zoom >= GROUND.cell()), capacity(cell_zoom));
                let boxes = words / words_a_box;
                let asked = BuildEvent::Reserve {
                    id,
                    generation: job.generation,
                    boxes,
                };
                if !emit(asked) {
                    self.bake_job = None;
                    self.bake.abort();
                    self.unwant(id);
                    return Progress::Busy;
                }
                let block = boxes.min(room_a_slot) * words_a_box;
                let Some(mesh_at) = at(Range::Bake, 0, block * 4) else {
                    self.abort_bake();
                    self.refuse(id);
                    return Progress::Blocked;
                };
                // SAFETY: `.fixed.bake` is the open bake's alone until finished or aborted.
                unsafe { self.bake.provide_mesh(ptr_at::<u32>(mesh_at as usize), block) };
                self.bake_job.as_mut().unwrap().mesh = Some((mesh_at as usize, block * 4));
                Progress::Busy
            }
            Step::Full => {
                let job = self.bake_job.as_ref().unwrap();
                let (at, bytes) = job.mesh.unwrap();
                if gpu().is_some() && device_ready() && !room(bytes) {
                    return Progress::Blocked;
                }
                let words_a_box = box_words(c.at.cell_zoom() >= GROUND.cell());
                let n = bytes / 4 / words_a_box;
                emit(BuildEvent::Part {
                    id,
                    generation: job.generation,
                    at: at as u32,
                    first: job.handed,
                    box_cnt: n,
                    heights: self.bake.heights(),
                });
                let job = self.bake_job.as_mut().unwrap();
                job.handed += n;
                let left = self.bake.left_words();
                let block = left.min(bytes / 4);
                // SAFETY: the same block of `.fixed.bake`, its boxes handed.
                unsafe { self.bake.provide_mesh(ptr_at::<u32>(at), block) };
                Progress::Busy
            }
            Step::Done => {
                let job = self.bake_job.as_ref().unwrap();
                // An empty mesh asked for no block and is handed over as zero bytes.
                let (at, bytes) = job.mesh.unwrap_or((0, 0));
                // No room in this turn's upload stream: wait for the next turn. With no
                // device there is no stream, and the hand-over goes on.
                if gpu().is_some() && device_ready() && !room(bytes) {
                    return Progress::Blocked;
                }
                let mesh = self.bake.finish();
                let job = self.bake_job.take().unwrap();
                self.stat.bakes += 1;
                if mesh.box_cnt > job.handed {
                    emit(BuildEvent::Part {
                        id,
                        generation: job.generation,
                        at: at as u32,
                        first: job.handed,
                        box_cnt: mesh.box_cnt - job.handed,
                        heights: mesh.heights,
                    });
                }
                self.unwant(id);
                emit(BuildEvent::Tier {
                    id,
                    generation: job.generation,
                    box_cnt: mesh.box_cnt,
                    heights: mesh.heights,
                });
                Progress::Busy
            }
        }
    }
}
