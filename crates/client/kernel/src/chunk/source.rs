// pxl-kernel/chunk/source.rs

//! Which written cubes a chunk draws, and what of the objects it draws: the
//! catalog numbers it reads, so a chunk is remade only where they differ.

use crate::layer::Query;
use pxl_render::mesh::run::APRON;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::aabb::Aabb;

use std::collections::HashMap;

use crate::chunk::build::runs_of;
use crate::chunk::Chunk as PyramidChunk;
use crate::layer::object::store::Store;
use crate::with_parts;
use pxl_spec::space::coord::world::nearest_box;
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use xpute_guest::global::Global;

static SOURCES_CHANGED: Global<bool> = Global::new();

/// The table compares every chunk's sources at its next turn (`sources_step`).
pub fn chunk_sources_changed() {
    *SOURCES_CHANGED.get(|| false) = true;
}

pub fn chunk_sources_step() {
    if core::mem::take(SOURCES_CHANGED.get(|| false)) {
        with_parts(|_, sched, _| sched.sources_step());
    }
}

/// The chunk with a cell of apron across, what its runs' fold reads (`Store::fold_at`).
fn reach(k: PyramidChunk) -> Aabb {
    let (cell, r) = (units(k.cell_zoom()), k.region());
    Aabb {
        min: [r.min[0] - cell, r.min[1] - cell, r.min[2]],
        max: [r.max[0] + cell, r.max[1] + cell, r.max[2]],
    }
}

/// Whether `k` draws the cube at `e`.
pub fn draws(k: PyramidChunk, e: EntryAt) -> bool {
    let r = reach(k);
    let b = nearest_box(&cube(e.zoom(), e.at()), (r.min[0] + r.max[0]) / 2.0);
    (0..3).all(|a| b.min[a] < r.max[a] && b.max[a] > r.min[a])
}

/// What a chunk's runs draw of the objects: the catalog number at every
/// place of their grid (`Cells::column_cells`), VOID where none is listed. A place
/// and its number are one word, sorted; none where the heap refused it.
#[derive(Clone, PartialEq, Eq)]
pub struct Drawn {
    runs: Zoom,
    cells: Vec<u64>,
}

impl Drawn {
    /// The zoom the chunk's runs are written at.
    pub fn runs(&self) -> Zoom {
        self.runs
    }
}

pub fn objects_drawn(store: &Store, k: PyramidChunk) -> Option<Drawn> {
    let runs = runs_of(store, k);
    let mut cells = Vec::new();
    let objects = store.read(k.node(), runs).value()?;
    let (from, _) = objects.column_cells(i64::from(APRON));
    let mut folded = HashMap::new();
    objects.fold(i64::from(APRON), None, &mut folded).ok()?;
    cells.try_reserve_exact(folded.len()).ok()?;
    // X and Y within the grid fit a byte each (`Cells::column_cells`); Z is the lane's.
    cells.extend(
        folded
            .into_iter()
            .map(|([x, y, z], v)| (x - from[0]) as u64 | ((y - from[1]) as u64) << 8 | u64::from(z as i32 as u32) << 16 | u64::from(v as u16) << 48),
    );
    cells.sort_unstable();
    Some(Drawn { runs, cells })
}

/// One leaf a chunk's decor stands among: its zoom, catalog number and place
/// from the chunk's corner at its zoom.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Met {
    zoom: Zoom,
    value: u16,
    at: [i32; 3],
}

/// Every leaf meeting the chunk, air included, sorted; none where the heap refused it.
pub fn objects_met(store: &Store, k: PyramidChunk) -> Option<Vec<Met>> {
    let (zoom, corner) = k.node();
    let mut out: Vec<Met> = Vec::new();
    let mut refused = false;
    store.read(k.node(), k.cell_zoom()).value()?.entries(&mut |e| {
        let from = if e.zoom >= zoom {
            first_descendant_of(corner, u32::from(e.zoom - zoom))
        } else {
            ancestor_of(corner, u32::from(zoom - e.zoom))
        };
        refused |= out.try_reserve(1).is_err();
        if !refused {
            out.push(Met {
                zoom: e.zoom,
                value: e.state as u16,
                at: core::array::from_fn(|a| (e.at[a] - from[a]) as i32),
            });
        }
    });
    (!refused).then(|| {
        out.sort_unstable();
        out
    })
}
