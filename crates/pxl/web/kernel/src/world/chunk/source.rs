// pxl-kernel/world/chunk/source.rs

//! What a chunk is made of past its page: which columns' objects and which
//! bands it draws, and their revisions. A chunk is a cube, so an object
//! written is in one chunk per level, not every chunk over its column.

use pxl_spec::bake::constant::BandZ;
use pxl_spec::space::hierarchy::ancestor_of;
use pxl_spec::space::indexer::{FROM_L2_IDX, L1, L2, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND, TOP_LEVEL};

use crate::kernel::with_world;
use crate::world::change::{bump_world, world_changes, TIERS_CHUNK};
use crate::world::pyramid::chunk::Chunk as PyramidChunk;
use xpute_runtime::global::Global;

static SOURCES_CHANGED: Global<bool> = Global::new();

/// The table compares every chunk's sources at its next turn (`sources_step`).
pub fn chunk_sources_changed() {
    *SOURCES_CHANGED.get(|| false) = true;
}

pub fn chunk_sources_step() {
    if core::mem::take(SOURCES_CHANGED.get(|| false)) {
        with_world(|world, sched| sched.sources_step(world));
    }
}

/// A leaf draws its own column; a level chunk draws every column under it
/// and its apron, folded.
pub fn tiers_draw(level: u32, base: IVec, col: IVec) -> bool {
    if level == 0 {
        col == L2_IDX(base)
    } else {
        folds_into(level, base, col)
    }
}

/// A band is a z16 chunk's height, so it lies in one chunk per level.
pub fn draws_band(k: PyramidChunk, band: BandZ) -> bool {
    ancestor_of([i64::from(band)], k.level())[0] == k.node().1[2]
}

/// One word: the coordinate, with the level beside the height. A band is 64
/// units of a ±32,768 lane, so the height fits 11 bits and the level the 4 above.
fn chunk_key(k: PyramidChunk) -> i64 {
    let [x, y, z] = k.node().1;
    ivec::gather(x, y, (k.level() as i64) << 11 | (z & 0x7ff)) as i64
}

const _: () = assert!((TOP_LEVEL as i64 + 1) << 11 <= 1 << 15, "a chunk's level and height past the lane they share");

pub fn objects_revision_of(k: PyramidChunk) -> u32 {
    world_changes().revision_of(TIERS_CHUNK, chunk_key(k))
}

/// Moves exactly the chunks whose tiers draw the band: the one holding it at
/// each level, and above z16 its neighbours through the fold's apron.
pub fn band_written(col: IVec, band: BandZ) {
    let corner = Pvf3 { i: FROM_L2_IDX(col), f: 0 };
    for level in 0..=TOP_LEVEL {
        let column = PyramidChunk::of(level, &corner);
        let home = column.beside(0, 0, ancestor_of([i64::from(band)], level)[0] - column.node().1[2]);
        let near: &[(i64, i64)] = if level == 0 { &[(0, 0)] } else { &NEAR };
        for &(dx, dy) in near {
            let k = home.beside(dx, dy, 0);
            if tiers_draw(level, k.corner(), col) {
                bump_world(TIERS_CHUNK, chunk_key(k) as u64);
            }
        }
    }
}

const NEAR: [(i64, i64); 9] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// Its own cells and the one-cell apron a fold reads.
fn folds_into(level: u32, base: IVec, col: IVec) -> bool {
    let (cell, o, last) = (GROUND.scale(level), FROM_L2_IDX(col), i64::from(L1.count_in(L2)) - 1);
    let across = |b: i64, o: i64| cell.index(o) <= cell.index(b) + CHUNK_CELLS as i64 && cell.index(o + last) >= cell.index(b) - 1;
    across(ivec::X(base), ivec::X(o)) && across(ivec::Y(base), ivec::Y(o))
}
