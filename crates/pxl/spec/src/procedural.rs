// pxl-spec/procedural.rs

//! What the client makes and the bake never stores: scattered decor, bridge
//! piers. Pure functions of the world and a seed, so every reader agrees.

use std::collections::TryReserveError;

use xpute_core::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};
use xpute_core::math::scalar::round;

use crate::bake::deck::{Occupies, Over};
use crate::bake::format::GroundCell;
use crate::cell::cube::cube_types::{sentinel, Ground};
use crate::cell::cube::Kind;
use crate::space::adapter::epsg3857::u_per_m;
use crate::space::pvf3::ivec::gather;
use crate::space::pyramid::{Tile, GROUND, LEAF_Z, TILE_CELLS, TOP_Z};
use crate::space::zoom::Zoom;

/// Changing it reshuffles everything scattered.
pub const MASTER_SEED: u64 = 0xd0c0_dec0_5eed_0001;

// ---- scatter: a Poisson process on the leaf cell lattice ----

const BLOCK: i64 = 16; // a block's side, in its birth zoom's cells
const KEPT: f64 = 0.25; // a zoom keeps one in four of the next finer's

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Candidate {
    pub cell: [i64; 2], // the leaf cell it stands at, at every zoom
    pub born: Zoom,     // the coarsest zoom it stands at
    pub seed: u64,      // its own draws: variant, heading, size
}

/// A Poisson process, never a grid: a block at zoom m bears a Poisson count
/// of candidates born there, each at a uniform leaf cell, and a candidate born
/// at m stands at every zoom from m finer. A cell at any zoom then holds `density`.
fn each_candidate(tag: u64, density: f64, zoom: Zoom, [min, max]: [[i64; 2]; 2], mut each: impl FnMut(Candidate)) {
    let kind = derive_seed_u64(MASTER_SEED, tag);
    for born in (TOP_Z..=zoom).rev() {
        let side = BLOCK * i64::from(GROUND.finest_in(born));
        let share = if born == TOP_Z { 1.0 } else { 1.0 - KEPT };
        let mean = density * (BLOCK * BLOCK) as f64 * share;
        let blocks = |k: usize| min[k].div_euclid(side)..=(max[k] - 1).div_euclid(side);
        for by in blocks(1) {
            for bx in blocks(0) {
                let block = derive_seed_u64(derive_seed_u64(kind, gather(bx, by, 0)), u64::from(born));
                let mut draws = SplitMix64::new(block);
                let mut unit = || u32_to_unit(draws.next_u32());
                for i in 0..poisson(mean, unit()) {
                    let cell = [bx * side + (unit() * side as f64) as i64, by * side + (unit() * side as f64) as i64];
                    if (0..2).all(|k| (min[k]..max[k]).contains(&cell[k])) {
                        each(Candidate {
                            cell,
                            born,
                            seed: derive_seed_u64(block, i),
                        });
                    }
                }
            }
        }
    }
}

/// The count whose cumulative Poisson probability first reaches `u`.
fn poisson(mean: f64, u: f64) -> u64 {
    let (mut k, mut p) = (0, libm::exp(-mean));
    let mut cumulative = p;
    while u >= cumulative && p > 0.0 {
        k += 1;
        p *= mean / k as f64;
        cumulative += p;
    }
    k
}

/// What stands at `z` in `region` (leaf cells, `[min, max)`), one a cell:
/// of two on one cell the coarser stays, so a zoom keeps what it had.
pub fn scatter(tag: u64, density: f64, zoom: Zoom, region: [[i64; 2]; 2], out: &mut Vec<Candidate>) -> Result<(), TryReserveError> {
    out.clear();
    let mut grown = Ok(());
    each_candidate(tag, density, zoom, region, |c| {
        if grown.is_ok() {
            grown = out.try_reserve(1).map(|()| out.push(c));
        }
    });
    grown?;
    out.sort_unstable_by(|a, b| a.cell.cmp(&b.cell).then(a.born.cmp(&b.born)).then(a.seed.cmp(&b.seed)));
    out.dedup_by_key(|c| c.cell);
    Ok(())
}

/// What stands in one leaf cell on the leaves, as `scatter` keeps it.
pub fn scatter_at(tag: u64, density: f64, cell: [i64; 2]) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    each_candidate(tag, density, LEAF_Z, [cell, [cell[0] + 1, cell[1] + 1]], |c| {
        if best.is_none_or(|b| (c.born, c.seed) < (b.born, b.seed)) {
            best = Some(c);
        }
    });
    best
}

// ---- piers ----

/// Within Arnis's 19 m and OSM2World's 50 m; one lattice a z5 row.
pub const PIER_STEP_M: f64 = 32.0;

pub const PIER_ACROSS: f64 = 0.7; // of the deck's width: OSM2World's `BridgeModule`
pub const PIER_ALONG: f64 = PIER_ACROSS / 2.0; // OSM2World's

/// A tile's cells in the world, so a station is the world's and not a tile's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub corner: [f64; 2], // south-west, in u
    pub cell: f64,        // a cell's side, in u
}

impl Frame {
    pub fn of(tile: Tile) -> Frame {
        let (x0, y0, side) = tile.rect();
        Frame {
            corner: [x0, y0],
            cell: side / f64::from(TILE_CELLS),
        }
    }
}

/// Whether `at` is in the pier at the station nearest `near` on `[a, a + d)`,
/// in the tile's cells; half-open, so a joint's station is one segment's.
pub fn pier_covers(frame: Frame, width: f64, a: [f64; 2], d: [f64; 2], near: [f64; 2], at: [f64; 2]) -> bool {
    let k = if d[0].abs() >= d[1].abs() { 0 } else { 1 };
    if d[k] == 0.0 {
        return false;
    }
    let lattice = PIER_STEP_M * u_per_m(frame.corner[1]);
    let station = round((frame.corner[k] + near[k] * frame.cell) / lattice) * lattice;
    let t = ((station - frame.corner[k]) / frame.cell - a[k]) / d[k];
    if !(0.0..1.0).contains(&t) {
        return false;
    }
    let len = libm::hypot(d[0], d[1]);
    let dir = [d[0] / len, d[1] / len];
    let rel = [at[0] - (a[0] + d[0] * t), at[1] - (a[1] + d[1] * t)];
    let along = (rel[0] * dir[0] + rel[1] * dir[1]).abs();
    let across = (rel[0] * dir[1] - rel[1] * dir[0]).abs();
    // At least the station's own cell.
    along <= (PIER_ALONG * width / 2.0).max(0.5) && across <= (PIER_ACROSS * width / 2.0).max(0.5)
}

/// None on a way or a building at grade (Arnis `support_blocked`, OSM2World),
/// none over a lower deck or ramp (Arnis), none where the terrain gives no
/// surface to stand on; water stops none.
pub fn pier_stands(pier: [i32; 2], cell: GroundCell, column: impl IntoIterator<Item = Over>, ground: i32, cell_per_m: f64) -> Option<[i32; 2]> {
    if cell == sentinel::void || Ground::of(cell).is_some_and(|g| matches!(g.kind(), Kind::Transport | Kind::Structure)) {
        return None;
    }
    let lower = column.into_iter().any(|o| matches!(o.occupies(ground, cell_per_m), Occupies::Above { slab, .. } if slab[1] <= pier[1]));
    (!lower).then_some(pier)
}

#[cfg(test)]
#[path = "procedural.test.rs"]
mod test;
