// pxl-kernel/chunk/source.rs

//! Which written cubes a chunk draws: where the objects are written, a chunk
//! that draws them is remade.

use crate::layer::Query;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::procedural::opened_by;
use pxl_spec::space::coord::aabb::Aabb;

use crate::chunk::Chunk as PyramidChunk;
use crate::layer::object::preview::RenderEdits;
use crate::layer::object::store::Store;
use crate::with_parts;
use pxl_spec::space::coord::world::nearest_box;
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use xpute_guest::global::Global;

static SOURCES_CHANGED: Global<bool> = Global::new();

/// The preview the pointer's edit hands the chunks, the one copy of it.
static PREVIEW: Global<RenderEdits> = Global::new();

/// The pointer's edit hands its preview here once a turn; whether it changed,
/// in which case the chunks compare their sources at their next turn.
pub fn preview(edits: RenderEdits) -> bool {
    let changed = core::mem::replace(PREVIEW.get(RenderEdits::default), edits) != edits;
    if changed {
        chunk_sources_changed();
    }
    changed
}

/// What the drawn build leaves out now.
pub fn previewed() -> RenderEdits {
    *PREVIEW.get(RenderEdits::default)
}

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
    draws_in(k, &cube(e.zoom(), e.at()))
}

/// Whether `k` draws anything in `b`.
fn draws_in(k: PyramidChunk, b: &Aabb) -> bool {
    let r = reach(k);
    let b = nearest_box(b, (r.min[0] + r.max[0]) / 2.0);
    (0..3).all(|a| b.min[a] < r.max[a] && b.max[a] > r.min[a])
}

/// Where the objects were written since the chunks last compared their sources.
static WRITTEN: Global<Vec<Aabb>> = Global::new();

/// The objects were written in `regions`: a chunk that draws any of them, or
/// a cave they open or shut (`opened_by`), is remade, whether or not what it
/// folds them to changed.
pub fn objects_written(regions: &[Aabb]) {
    WRITTEN.get(Vec::new).extend(regions.iter().map(|&r| opened_by(r)));
    chunk_sources_changed();
}

/// Whether the objects were written where `k` draws, since the chunks last
/// compared their sources (`compared`).
pub fn written_in(k: PyramidChunk) -> bool {
    WRITTEN.get(Vec::new).iter().any(|b| draws_in(k, b))
}

/// The chunks compared their sources.
pub fn compared() {
    WRITTEN.get(Vec::new).clear();
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
