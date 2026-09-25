// pxl-kernel/world/chunk/source.rs

//! What a chunk is made of, past its page: which columns' objects it draws,
//! in its tiers and in its decor, the revisions those are read as, and the
//! flag that has the scheduler compare every chunk's against what it was
//! built from (table.rs `Scheduler::sources_step`).

use pxl_spec::space::indexer::{FROM_L2_IDX, L1, L2, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_span_of, CHUNK_CELLS, GROUND, TOP_LEVEL};

use crate::global::Global;
use crate::kernel::with_world;
use crate::layer::object::store::object_col_revision;
use crate::world::change::{bump_world, world_changes, DECOR_CHUNK, TIERS_CHUNK};
use crate::world::pyramid::chunk::Chunk as PyramidChunk;

static SOURCES_CHANGED: Global<bool> = Global::new();

/// A chunk's sources changed somewhere — a column written, a setting a chunk
/// paints with: the scheduler compares each chunk's own at its next turn
/// (table.rs `Scheduler::sources_step`).
pub fn chunk_sources_changed() {
    *SOURCES_CHANGED.get(|| false) = true;
}

pub fn chunk_sources_step() {
    if core::mem::take(SOURCES_CHANGED.get(|| false)) {
        with_world(|world, sched| sched.sources_step(world));
    }
}

/// Whether a chunk of `level` at `base` draws the objects of column `col`
/// in its tiers: its own column's for a leaf chunk, and every column under a
/// level chunk and its apron, since what it draws is all of theirs folded.
pub fn tiers_draw(level: u32, base: IVec, col: IVec) -> bool {
    if level == 0 {
        col == L2_IDX(base)
    } else {
        folds_into(level, base, col)
    }
}

/// Whether a chunk of `level` at `base` draws the placed decor of column
/// `col`: the columns whose corner lies in it.
pub fn decor_draws(level: u32, base: IVec, col: IVec) -> bool {
    let span = chunk_span_of(level) as i64;
    let o = FROM_L2_IDX(col);
    (ivec::X(base)..ivec::X(base) + span).contains(&ivec::X(o)) && (ivec::Y(base)..ivec::Y(base) + span).contains(&ivec::Y(o))
}

/// A chunk as the change log keys it: its coordinate among its level's
/// chunks and its level, in one word.
fn chunk_key(level: u32, base: IVec) -> i64 {
    let [x, y] = PyramidChunk::of(level, &Pvf3 { i: base, f: 0 }).node().1;
    ivec::gather(x, y, level as i64) as i64
}

/// What a chunk's objects are made of, as a revision: its column's for a
/// leaf chunk, and for a level chunk the one every column its tiers draw
/// moves (`column_written`).
pub fn objects_revision_of(level: u32, base: IVec) -> u32 {
    if level == 0 {
        return object_col_revision(L2_IDX(base));
    }
    world_changes().revision_of(TIERS_CHUNK, chunk_key(level, base))
}

/// What a chunk's placed decor is made of, as a revision, the same way.
pub fn decor_revision_of(level: u32, base: IVec) -> u32 {
    if level == 0 {
        return object_col_revision(L2_IDX(base));
    }
    world_changes().revision_of(DECOR_CHUNK, chunk_key(level, base))
}

/// A write to column `col`: every chunk above z16 that draws it moves, in
/// its tiers or its decor. Its tiers' are the chunk the column lies in and,
/// through the apron a fold reads, its neighbours; its decor's the one it
/// lies in.
pub fn column_written(col: IVec) {
    let corner = Pvf3 { i: FROM_L2_IDX(col), f: 0 };
    for level in 1..=TOP_LEVEL {
        let home = PyramidChunk::of(level, &corner);
        for (dx, dy) in (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (dx, dy))) {
            let base = home.beside(dx, dy).base().i;
            if tiers_draw(level, base, col) {
                bump_world(TIERS_CHUNK, chunk_key(level, base) as u64);
            }
            if decor_draws(level, base, col) {
                bump_world(DECOR_CHUNK, chunk_key(level, base) as u64);
            }
        }
    }
}

/// Whether the objects of column `col` fold into the level-`level` chunk at
/// `base`: its own cells, and the one-cell apron around them a fold reads.
fn folds_into(level: u32, base: IVec, col: IVec) -> bool {
    // In cells of the level: the chunk's with its apron, and the first and
    // last a column, a leaf chunk, reaches.
    let (cell, o, last) = (GROUND.scale(level), FROM_L2_IDX(col), i64::from(L1.count_in(L2)) - 1);
    let across = |b: i64, o: i64| cell.index(o) <= cell.index(b) + CHUNK_CELLS as i64 && cell.index(o + last) >= cell.index(b) - 1;
    across(ivec::X(base), ivec::X(o)) && across(ivec::Y(base), ivec::Y(o))
}
