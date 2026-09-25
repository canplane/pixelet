// pxl-kernel/world/chunk/source.rs

//! What a chunk is made of, past its page: which columns' objects it draws,
//! in its tiers and in its decor, and of those which bands — a chunk is a
//! cube, so an object written is in one chunk a level and not in every chunk
//! over its column — the revisions those are read as, and the
//! flag that has the scheduler compare every chunk's against what it was
//! built from (table.rs `Scheduler::sources_step`).

use pxl_spec::bake::constant::BandZ;
use pxl_spec::space::indexer::{FROM_L2_IDX, L1, L2, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_span_of, CHUNK_CELLS, GROUND, TOP_LEVEL};

use crate::global::Global;
use crate::kernel::with_world;
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

/// Whether chunk `k` draws what lies in band `band` of a column: a band is a
/// z16 chunk's height, so a band is in the chunk of any level whose height
/// holds it, one a level.
pub fn draws_band(k: PyramidChunk, band: BandZ) -> bool {
    i64::from(band) >> k.level() == k.node().1[2]
}

/// A chunk as the change log keys it, in one word: its coordinate among its
/// level's chunks, and its level beside its height in the height's lane. A
/// chunk's height index is a band's shifted by its level, and a band is 64
/// units of a lane of ±32,768, so eleven bits hold it and the level takes the
/// four over them.
fn chunk_key(k: PyramidChunk) -> i64 {
    let [x, y, z] = k.node().1;
    ivec::gather(x, y, (k.level() as i64) << 11 | (z & 0x7ff)) as i64
}

const _: () = assert!((TOP_LEVEL as i64 + 1) << 11 <= 1 << 15, "a chunk's level and height past the lane they share");

/// What a chunk's objects are made of, as a revision: the one every band its
/// tiers draw moves (`band_written`).
pub fn objects_revision_of(k: PyramidChunk) -> u32 {
    world_changes().revision_of(TIERS_CHUNK, chunk_key(k))
}

/// What a chunk's placed decor is made of, as a revision, the same way.
pub fn decor_revision_of(k: PyramidChunk) -> u32 {
    world_changes().revision_of(DECOR_CHUNK, chunk_key(k))
}

/// A write to band `band` of column `col`: every chunk that draws it moves,
/// in its tiers or its decor, and no other — a chunk over the column that
/// does not hold the band is not made again. Its tiers' are the chunk the
/// band lies in at each level and, above z16, through the apron a fold reads,
/// its neighbors at that height; its decor's the one it lies in.
pub fn band_written(col: IVec, band: BandZ) {
    let corner = Pvf3 { i: FROM_L2_IDX(col), f: 0 };
    for level in 0..=TOP_LEVEL {
        let column = PyramidChunk::of(level, &corner);
        let home = column.beside(0, 0, (i64::from(band) >> level) - column.node().1[2]);
        let near: &[(i64, i64)] = if level == 0 { &[(0, 0)] } else { &NEAR };
        for &(dx, dy) in near {
            let k = home.beside(dx, dy, 0);
            if tiers_draw(level, k.corner(), col) {
                bump_world(TIERS_CHUNK, chunk_key(k) as u64);
            }
            if decor_draws(level, k.corner(), col) {
                bump_world(DECOR_CHUNK, chunk_key(k) as u64);
            }
        }
    }
}

/// A chunk and the eight beside it.
const NEAR: [(i64, i64); 9] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// Whether the objects of column `col` fold into the level-`level` chunk at
/// `base`: its own cells, and the one-cell apron around them a fold reads.
fn folds_into(level: u32, base: IVec, col: IVec) -> bool {
    // In cells of the level: the chunk's with its apron, and the first and
    // last a column, a leaf chunk, reaches.
    let (cell, o, last) = (GROUND.scale(level), FROM_L2_IDX(col), i64::from(L1.count_in(L2)) - 1);
    let across = |b: i64, o: i64| cell.index(o) <= cell.index(b) + CHUNK_CELLS as i64 && cell.index(o + last) >= cell.index(b) - 1;
    across(ivec::X(base), ivec::X(o)) && across(ivec::Y(base), ivec::Y(o))
}
