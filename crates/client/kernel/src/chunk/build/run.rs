// pxl-kernel/chunk/build/run.rs

//! A column of chunks shares its runs: written once into a heap block, held while any chunk over it bakes.

use crate::layer::Query;
use pxl_spec::space::layer::lattice::GROUND;

use crate::param::BAKE_ROWS_PER_STEP;
use pxl_render::mesh::paint::column::{stands_of, write_rows};
use pxl_render::mesh::run::{RunSet, APRON};
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};

use crate::chunk::source::draws;

use pxl_spec::space::coord::zoom::{units, Zoom};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::*;

// A chunk's runs live in a heap block exactly as large as its column needs,
// held until all its tiers are baked. Not a `.fixed` range: a column's runs
// are unbounded, and a range too small would leave a chunk undrawn; the heap
// refuses only when memory is out, and then the build is `Blocked`.

pub(super) const RUN_SETS: usize = 2;

/// A column of chunks one above another shares its runs: they are made once
/// and each chunk bakes its cut of them.
pub(super) struct RunHold {
    pub(super) column: (Zoom, [i64; 2]),
    pub(super) users: Vec<u32>,
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
    /// What they were written at, which the data decided (`runs_zoom`).
    pub(super) zoom: Zoom,
}

/// The one chunk whose runs are being written, a few rows a step: its sample
/// and what stands in it are the builder's until it closes.
pub(super) struct RunsJob {
    pub(super) id: u32,
    pub(super) zoom: Zoom,
    pub(super) row: u32,
    pub(super) next: usize,
}

impl Builder {
    pub(super) fn hold_of(&self, id: u32) -> Option<usize> {
        let column = self.chunks.get(&id)?.at.column();
        self.run_holds.iter().position(|h| h.as_ref().is_some_and(|h| h.column == column))
    }

    pub(super) fn runs_of(&self, id: u32) -> Option<Runs> {
        self.run_holds[self.hold_of(id)?].as_ref()?.runs
    }

    pub(super) fn run_set_for(&mut self, id: u32) -> Option<usize> {
        if let Some(set) = self.hold_of(id) {
            let users = &mut self.run_holds[set].as_mut()?.users;
            if !users.contains(&id) {
                users.push(id);
            }
            return Some(set);
        }
        let set = self.run_holds.iter().position(Option::is_none)?;
        let column = self.chunks.get(&id)?.at.column();
        self.run_holds[set] = Some(RunHold { column, users: vec![id], runs: None });
        Some(set)
    }

    pub(super) fn leave_runs(&mut self, id: u32) {
        for slot in self.run_holds.iter_mut() {
            if let Some(h) = slot {
                h.users.retain(|&u| u != id);
                if h.users.is_empty() {
                    *slot = None;
                }
            }
        }
    }

    /// A job writing the column's runs from what it read before is let go too.
    pub(super) fn drop_runs(&mut self, id: u32) {
        let column = |b: &Builder, id: u32| b.chunks.get(&id).map(|c| c.at.column());
        let same = self.runs_job.as_ref().is_some_and(|j| column(self, j.id).is_some() && column(self, j.id) == column(self, id));
        self.end_runs_job(|_| same);
        if let Some(set) = self.hold_of(id) {
            self.run_holds[set] = None;
        }
        self.leave_runs(id);
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
        let target = runs_of(build.store, c.at);
        let Some(read) = c.read(target) else {
            return Progress::Blocked;
        };
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let (k, span) = (c.finer(target), run_span(c.at, target));
        let sampled = if k == 0 {
            self.sample.read(&read.around(), read.first, span)
        } else {
            let leaf = c.leaf(target);
            let leaf_span = run_span(leaf, GROUND.cell_zoom(GROUND.read_zoom(leaf.cell_zoom())));
            self.leaf_sample
                .read(&read.around(), read.first, leaf_span)
                .and_then(|()| self.sample.refine(&self.leaf_sample, k, c.in_leaf(target), span))
        };
        if sampled.is_err() {
            self.refuse(id);
            return Progress::Blocked;
        }
        let edits = crate::chunk::source::previewed();
        self.folded.clear();
        let objects = build.store.read(c.at.node(), target).value().or_bug(Errno::ENOTRECOVERABLE);
        let (from, _) = objects.column_cells(i64::from(APRON));
        let (cx, cy) = (from[0] + i64::from(APRON), from[1] + i64::from(APRON));
        if objects.fold(i64::from(APRON), edits.hide.filter(|&e| draws_as_held(c.at, e.zoom())), &mut self.folded).is_err() {
            self.refuse(id);
            return Progress::Blocked;
        }
        if let Some(carve) = edits.carve.filter(|&e| draws(c.at, e) && draws_as_held(c.at, e.zoom())) {
            carve_into(&mut self.folded, carve, target, from[0]);
        }
        let step = (units(target) / z_unit(target)) as u32;
        if stands_of(&self.sample, build.painter, &self.folded, (cx, cy), step, &mut self.stands).is_err() {
            self.refuse(id);
            return Progress::Blocked;
        }
        xpute_kit::ensure!(self.direct.begin(span), ENOTRECOVERABLE);
        self.runs_job = Some(RunsJob { id, zoom: target, row: 0, next: 0 });
        Progress::Busy
    }

    pub(super) fn write_runs(&mut self, id: u32) -> Progress {
        let job = self.runs_job.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        let span = self.sample.span;
        let rows = job.row..span.min(job.row + BAKE_ROWS_PER_STEP);
        if write_rows(&mut self.direct, &self.sample, &self.stands, rows.clone(), &mut job.next).is_err() {
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
