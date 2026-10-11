// pxl-kernel/chunk/build/run.rs

//! A chunk's runs, its own heights composed: written once into a heap block, held while it bakes.

use crate::layer::Base as _;
use crate::layer::Query;
use crate::param::BAKE_ROWS_PER_STEP;
use crate::query::World;
use pxl_render::mesh::paint::column::write_rows;
use pxl_render::mesh::run::RunSet;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::block::sample::APRON;
use pxl_spec::block::stand::z_unit;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};

use crate::chunk::source::draws;

use pxl_spec::space::coord::zoom::Zoom;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::*;

// A chunk's runs live in a heap block exactly as large as its column needs,
// held until all its tiers are baked. Not a `.fixed` range: a column's runs
// are unbounded, and a range too small would leave a chunk undrawn; the heap
// refuses only when memory is out, and then the build is `Blocked`.

pub(super) const RUN_SETS: usize = 2;

/// A chunk's runs, made once and baked into each of its tiers.
pub(super) struct RunHold {
    pub(super) id: u32,
    pub(super) runs: Option<Runs>,
}

impl Drop for RunHold {
    fn drop(&mut self) {
        if let Some(r) = self.runs.take() {
            // SAFETY: the hold's own block; every bake from it ends before the hold does.
            unsafe { free(r.block) };
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Runs {
    pub(super) block: (usize, u32),
    pub(super) bytes: u32,
    pub(super) count: u32,
    /// What they were written at, which the data decided (`query::runs`).
    pub(super) zoom: Zoom,
}

/// The one chunk whose runs are being written, a few rows a step: its sample
/// and what stands in it are the builder's until it closes.
pub(super) struct RunsJob {
    pub(super) id: u32,
    pub(super) zoom: Zoom,
    /// The chunk's heights, in its runs' cells.
    pub(super) range: [i32; 2],
    pub(super) row: u32,
    pub(super) next: usize,
}

impl Builder {
    pub(super) fn hold_of(&self, id: u32) -> Option<usize> {
        self.run_holds.iter().position(|h| h.as_ref().is_some_and(|h| h.id == id))
    }

    pub(super) fn runs_of(&self, id: u32) -> Option<Runs> {
        self.run_holds[self.hold_of(id)?].as_ref()?.runs
    }

    pub(super) fn run_set_for(&mut self, id: u32) -> Option<usize> {
        if let Some(set) = self.hold_of(id) {
            return Some(set);
        }
        let set = self.run_holds.iter().position(Option::is_none)?;
        self.run_holds[set] = Some(RunHold { id, runs: None });
        Some(set)
    }

    pub(super) fn leave_runs(&mut self, id: u32) {
        if let Some(set) = self.hold_of(id) {
            self.run_holds[set] = None;
        }
    }

    pub(super) fn end_runs_job(&mut self, which: impl Fn(&RunsJob) -> bool) {
        if self.runs_job.as_ref().is_some_and(which) {
            self.runs_job = None;
            self.direct.abort();
        }
    }

    pub fn run_sets(&self) -> (u32, u32) {
        let held = self.run_holds.iter().flatten();
        let bytes = held.clone().filter_map(|h| h.runs.map(|r| r.bytes)).sum();
        (held.count() as u32, bytes)
    }

    /// A chunk's runs from its tile's ground and the objects folded to its
    /// runs' cells (mesh/paint/column.rs): the sources at once, then a few rows
    /// a step. One chunk's at a time.
    pub(super) fn advance_runs(&mut self, build: &mut Build, id: u32) -> Progress {
        match &self.runs_job {
            Some(j) if j.id != id => return Progress::Blocked,
            Some(_) => return self.write_runs(id),
            None => {}
        }
        let c = self.chunks[&id];
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let edits = crate::chunk::source::previewed();
        let hide = edits.hide.filter(|&e| draws_as_held(c.at, e.zoom()));
        let carve = edits.carve.filter(|&e| draws(c.at, e) && draws_as_held(c.at, e.zoom()));
        let world = World::new(TerrainTile::residency(), build.store).previewing(hide, carve);
        let Some(column) = world.read(c.at.node(), c.at.cell_zoom()).value() else {
            return Progress::Blocked;
        };
        let target = column.runs;
        // And the cell under its floor, which decides the floor's faces.
        let [lo, hi] = window(c.at, target);
        let range = [lo - 1, hi];
        if column.compose(&mut self.composer, &mut self.blocks).is_err() {
            self.refuse(id);
            return Progress::Blocked;
        }
        xpute_kit::ensure!(self.direct.begin(self.blocks.sample.span), ENOTRECOVERABLE);
        self.runs_job = Some(RunsJob {
            id,
            zoom: target,
            range,
            row: 0,
            next: 0,
        });
        Progress::Busy
    }

    pub(super) fn write_runs(&mut self, id: u32) -> Progress {
        let job = self.runs_job.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        let span = self.blocks.sample.span;
        let rows = job.row..span.min(job.row + BAKE_ROWS_PER_STEP);
        if write_rows(&mut self.direct, &self.blocks.sample, &self.blocks.stands, rows.clone(), job.range, &mut job.next).is_err() {
            self.runs_job = None;
            self.direct.abort();
            self.refuse(id);
            return Progress::Blocked;
        }
        job.row = rows.end;
        if rows.end < span {
            return Progress::Busy;
        }
        let zoom = job.zoom;
        self.runs_job = None;
        self.close(id, zoom)
    }

    pub(super) fn close(&mut self, id: u32, zoom: Zoom) -> Progress {
        let bytes = self.direct.close();
        let Some(block) = self.set_of(id, bytes) else {
            self.direct.abort();
            self.refuse(id);
            return Progress::Blocked;
        };
        // SAFETY: as for a scan's set.
        unsafe { self.direct.provide_set(ptr_at::<u8>(block.0), bytes) };
        let count = self.direct.finish();
        self.stat.runs += 1;
        self.keep_runs(id, Runs { block, bytes, count, zoom });
        Progress::Busy
    }

    pub(super) fn set_of(&mut self, id: u32, bytes: u32) -> Option<(usize, u32)> {
        self.run_set_for(id)?;
        let block = alloc(bytes);
        if block.is_none() {
            self.leave_runs(id);
        }
        block
    }

    pub(super) fn keep_runs(&mut self, id: u32, runs: Runs) {
        let set = self.hold_of(id).or_bug(Errno::ENOTRECOVERABLE);
        let hold = self.run_holds[set].as_mut().or_bug(Errno::ENOTRECOVERABLE);
        if let Some(old) = hold.runs.replace(runs) {
            // SAFETY: the earlier runs' block, which nothing bakes from once replaced.
            unsafe { free(old.block) };
        }
    }

    pub(super) fn run_set(runs: &Runs, span: u32) -> RunSet<'static> {
        let bytes = pxl_render::mesh::run::run_set_bytes(runs.count, span) as usize;
        xpute_kit::ensure!(bytes as u32 <= runs.block.1, ENOTRECOVERABLE);
        // SAFETY: the chunk's own block, held until its runs are let go.
        unsafe { RunSet::carve(core::slice::from_raw_parts(ptr_at::<u8>(runs.block.0), bytes), runs.count, span, APRON) }
    }
}

/// The chunk's own heights, `[lo, hi)`, in the cells of runs at `runs`.
pub(super) fn window(at: PyramidChunk, runs: Zoom) -> [i32; 2] {
    let (z0, z1) = at.z_range();
    let z = z_unit(runs);
    [(z0 / z) as i32, (z1 / z) as i32]
}
