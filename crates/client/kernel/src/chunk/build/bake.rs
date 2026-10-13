// pxl-kernel/chunk/build/bake.rs

//! A tier's bake: the mesh written a slot at a time into `.fixed.bake` and handed to the table.

use pxl_spec::space::layer::lattice::GROUND;

use pxl_render::gpu::api::{gpu, room};
use pxl_render::gpu::renderer::device_ready;
use pxl_render::mesh::chunk::{box_words, Step, COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::Block;

use crate::chunk::build::BAKE_RANGE;
use crate::layer::Base as _;
use crate::query::World;
use crate::setting::store::settings_store;

use crate::chunk::mesh_slot::{capacity, COARSE_BOXES, FINE_BOXES};

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

impl Builder {
    pub(super) fn abort_bake(&mut self) {
        if self.bake_job.take().is_some() {
            self.bake.abort();
        }
    }

    pub(super) fn advance_bake(&mut self, build: &mut Build, id: u32, emit: &mut impl FnMut(BuildEvent) -> bool) -> Progress {
        let holding = matches!(&self.bake_job, Some(j) if j.id == id);
        let c = self.chunks[&id];
        if !holding {
            if self.bake_job.is_some() {
                return Progress::Blocked;
            }
            // Its blocks as the world as drawn answers them, as the cast asked.
            let world = World::new(TerrainTile::residency(), build.store).previewed();
            let (cell, (zoom, at)) = (c.at.cell_zoom(), c.at.node());
            // What the rays stopped on in it, which alone is drawn.
            let seen = crate::chunk::domain::current().into_iter().flat_map(|d| d.seen_in(c.at));
            match world.compose((zoom, at), cell, seen, &mut self.blocks).value() {
                None => return Progress::Blocked,
                Some(Err(_)) => {
                    self.refuse(id);
                    return Progress::Blocked;
                }
                Some(Ok(())) => {}
            }
            self.bake.begin(cell, settings_store().textures, crate::param::BAKE_BOXES_PER_STEP);
            self.bake_job = Some(BakeJob {
                id,
                generation: c.generation,
                mesh: None,
                handed: 0,
            });
        }
        match self.bake.step(&self.blocks) {
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
                let Some(mesh_at) = BAKE_RANGE.get().at(0, block * 4) else {
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
