// pxl-kernel/world/chunk/source.rs

//! Which written cubes a chunk draws, and their revision. A chunk is a cube,
//! so a cube written is in the chunks over it at each zoom and their
//! neighbours through the fold's apron, not every chunk over its column.

use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::v3::Aabb;

use crate::kernel::with_world;
use crate::world::change::{bump_world, world_changes, CHUNK_OBJECTS};
use crate::world::pyramid::chunk::{Chunk as PyramidChunk, FINEST_CHUNK, TOP_CHUNK};
use pxl_spec::space::pyramid::CHUNK_STEPS;
use pxl_spec::space::zoom::{cube, nearest_box, units};
use xpute_runtime::global::Global;

static SOURCES_CHANGED: Global<bool> = Global::new();

/// The table compares every chunk's sources at its next turn (`sources_step`).
pub fn chunk_sources_changed() {
    *SOURCES_CHANGED.get(|| false) = true;
}

pub fn chunk_sources_step() {
    if core::mem::take(SOURCES_CHANGED.get(|| false)) {
        with_world(|_, sched| sched.sources_step());
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
    let b = nearest_box(&cube(e.zoom, e.at), (r.min[0] + r.max[0]) / 2.0);
    (0..3).all(|a| b.min[a] < r.max[a] && b.max[a] > r.min[a])
}

/// One word: the zoom, then each axis's index in a field wide enough for
/// the finest chunk's: an index at zoom z runs z bits across the world.
fn chunk_key(k: PyramidChunk) -> i64 {
    let (zoom, [x, y, h]) = k.named().node();
    (i64::from(zoom) << 58) | ((x & 0x3f_ffff) << 36) | ((y & 0x3f_ffff) << 14) | (h & 0x3fff)
}

const _: () = assert!(FINEST_CHUNK < 1 << 5 && FINEST_CHUNK <= 22, "a chunk's key past its fields");

pub fn objects_revision_of(k: PyramidChunk) -> u32 {
    world_changes().revision_of(CHUNK_OBJECTS, chunk_key(k))
}

/// Moves exactly the chunks that draw the cube at `e`.
pub fn written(e: EntryAt) {
    let b = cube(e.zoom, e.at);
    for zoom in TOP_CHUNK..=FINEST_CHUNK {
        let (side, cell) = (units(zoom), units(zoom + CHUNK_STEPS));
        let range = |a: usize, apron: f64| ((b.min[a] - apron) / side).floor() as i64..((b.max[a] + apron) / side).ceil() as i64;
        for h in range(2, 0.0) {
            for y in range(1, cell) {
                for x in range(0, cell) {
                    let k = PyramidChunk::indexed(zoom, [x, y, h]);
                    if draws(k, e) {
                        bump_world(CHUNK_OBJECTS, chunk_key(k) as u64);
                    }
                }
            }
        }
    }
}
