// pxl-kernel/world/chunk/source.rs

//! What a chunk is made of past its tile: which columns' objects and which
//! bands it draws, and their revisions. A chunk is a cube, so an object
//! written is in one chunk per zoom, not every chunk over its column.

use pxl_spec::bake::constant::{BandZ, BAND, BAND_UNITS};
use pxl_spec::space::hierarchy::ancestor_of;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;

use crate::kernel::with_world;
use crate::world::change::{bump_world, world_changes, TIERS_CHUNK};
use crate::world::pyramid::chunk::{Chunk as PyramidChunk, FINEST_CHUNK, TOP_CHUNK};
use pxl_spec::space::indexer::from_idx;
use pxl_spec::space::zoom::units;
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

/// A chunk draws the objects of every column its cells or the one-cell apron
/// its fold reads (`Store::fold_at`) lie over.
pub fn tiers_draw(k: PyramidChunk, col: IVec) -> bool {
    let (x0, y0, side) = k.rect();
    let cell = units(k.drawn_zoom());
    let o = from_idx(BAND, col);
    // The column's copy nearest the chunk, so a fold across the antimeridian is met.
    let ox = pxl_spec::space::pvf3::nearest(ivec::X(o) as f64, x0);
    let oy = ivec::Y(o) as f64;
    let across = |b: f64, o: f64| o < b + side + cell && o + BAND_UNITS > b - cell;
    across(x0, ox) && across(y0, oy)
}

/// Whether `k` draws a height of `band`: one chunk a zoom holds it up to
/// the band's, and past it the band holds several.
pub fn draws_band(k: PyramidChunk, band: BandZ) -> bool {
    let h = k.node().1[2];
    if k.zoom() <= BAND {
        ancestor_of([i64::from(band)], u32::from(BAND - k.zoom()))[0] == h
    } else {
        ancestor_of([h], u32::from(k.zoom() - BAND))[0] == i64::from(band)
    }
}

/// One word: the zoom, then each axis's index in a field wide enough for
/// the finest chunk's: an index at zoom z runs z bits across the world.
fn chunk_key(k: PyramidChunk) -> i64 {
    let (zoom, [x, y, h]) = k.named().node();
    (i64::from(zoom - TOP_CHUNK) << 58) | ((x & 0x3f_ffff) << 36) | ((y & 0x3f_ffff) << 14) | (h & 0x3fff)
}

const _: () = assert!(FINEST_CHUNK - TOP_CHUNK < 32 && FINEST_CHUNK <= 22, "a chunk's key past its fields");

pub fn objects_revision_of(k: PyramidChunk) -> u32 {
    world_changes().revision_of(TIERS_CHUNK, chunk_key(k))
}

/// Moves exactly the chunks whose tiers draw the band: those holding it at
/// each zoom, and their neighbours through the fold's apron.
pub fn band_written(col: IVec, band: BandZ) {
    let corner = Pvf3 { i: from_idx(BAND, col), f: 0 };
    for zoom in TOP_CHUNK..=FINEST_CHUNK {
        // Chunks a band column's side and height holds, at least one.
        let n = 1i64 << zoom.saturating_sub(BAND);
        let column = PyramidChunk::holding(zoom, &corner);
        let low = if zoom <= BAND {
            ancestor_of([i64::from(band)], u32::from(BAND - zoom))[0]
        } else {
            i64::from(band) << (zoom - BAND)
        };
        let home = column.beside(0, 0, low - column.node().1[2]);
        for dh in 0..n {
            for dy in -1..=n {
                for dx in -1..=n {
                    let k = home.beside(dx, dy, dh);
                    if tiers_draw(k, col) {
                        bump_world(TIERS_CHUNK, chunk_key(k) as u64);
                    }
                }
            }
        }
    }
}
