// pxl-spec/procedural.rs

//! What the client makes and the bake never stores: scattered decor, bridge
//! piers. Pure functions of the world and a seed, so every reader agrees.

use std::collections::TryReserveError;

use xpute_kit::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};
use xpute_kit::math::scalar::round;

use crate::bake::separation::{slab_depth, Crossing, Occupies, STACKED_DECK_HEADROOM_M};
use crate::bake::tile::terrain::GroundId;
use crate::bake::way::half_cells;
use crate::block::cube::Ground;
use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::pvf3::ivec::gather;
use crate::space::coord::zoom::{of_side, units, Zoom};
use crate::space::layer::lattice::{GROUND, TILE_CELLS};
use crate::space::layer::tile::{Tile, LEAF_ZOOM};

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
    for born in (GROUND.top()..=zoom).rev() {
        let side = BLOCK * i64::from(GROUND.finest_in(born));
        let share = if born == GROUND.top() { 1.0 } else { 1.0 - KEPT };
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

/// Whether `a` stands before `b` where both would claim a cell: the one born
/// coarser, so a zoom keeps what it had, then the lower seed.
fn before(a: &Candidate, b: &Candidate) -> bool {
    (a.born, a.seed) < (b.born, b.seed)
}

/// Whether the squares `side` gives two candidates, from their cells up, share a cell.
fn meet(a: &Candidate, b: &Candidate, side: &impl Fn(&Candidate) -> i64) -> bool {
    let (sa, sb) = (side(a), side(b));
    (0..2).all(|k| a.cell[k] < b.cell[k] + sb && b.cell[k] < a.cell[k] + sa)
}

/// What stands at `z` in `region` (leaf cells, `[min, max)`), by its cell.
/// Each candidate claims the square `side` gives it, from its cell up, which
/// holds its box however it turns, `widest` the largest; of two whose squares
/// meet, the one `before` the other stands.
pub fn scatter(tag: u64, density: f64, zoom: Zoom, region: [[i64; 2]; 2], widest: i64, side: impl Fn(&Candidate) -> i64, out: &mut Vec<Candidate>) -> Result<(), TryReserveError> {
    out.clear();
    let reach = widest - 1;
    let around = [region[0].map(|v| v - reach), region[1].map(|v| v + reach)];
    let mut grown = Ok(());
    each_candidate(tag, density, zoom, around, |c| {
        if grown.is_ok() {
            grown = out.try_reserve(1).map(|()| out.push(c));
        }
    });
    grown?;
    out.sort_unstable_by(|a, b| a.cell.cmp(&b.cell).then(a.born.cmp(&b.born)).then(a.seed.cmp(&b.seed)));
    // Survivors go past the end, then the whole is let go of.
    let n = out.len();
    for i in 0..n {
        let c = out[i];
        if !(0..2).all(|k| (region[0][k]..region[1][k]).contains(&c.cell[k])) {
            continue;
        }
        // Only a candidate within `reach` before it on x, or its own side past, can meet it.
        let from = out[..n].partition_point(|d| d.cell[0] < c.cell[0] - reach);
        let beaten = out[from..n].iter().take_while(|d| d.cell[0] < c.cell[0] + side(&c)).any(|d| before(d, &c) && meet(d, &c, &side));
        if !beaten {
            out.try_reserve(1)?;
            out.push(c);
        }
    }
    out.drain(..n);
    Ok(())
}

/// What stands in one leaf cell on the leaves, as `scatter` keeps it.
pub fn scatter_at(tag: u64, density: f64, cell: [i64; 2], widest: i64, side: impl Fn(&Candidate) -> i64) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    each_candidate(tag, density, LEAF_ZOOM, [cell, [cell[0] + 1, cell[1] + 1]], |c| {
        if best.is_none_or(|b| before(&c, &b)) {
            best = Some(c);
        }
    });
    let best = best?;
    let reach = widest - 1;
    let around = [cell.map(|v| v - reach), cell.map(|v| v + widest)];
    let mut beaten = false;
    each_candidate(tag, density, LEAF_ZOOM, around, |d| beaten |= before(&d, &best) && meet(&d, &best, &side));
    (!beaten).then_some(best)
}

// ---- piers ----

// From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
// the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
// on a deck's line rather than place blocks.
//
// A bridge the source names no structure for is Arnis's beam bridge
// (`pier_interval`, `place_pier_bent`): a bent every 8 m, 12 m once its half
// width passes two cells, of one-cell columns.
pub(crate) const BEAM_PILLAR_INTERVAL_M: f64 = 8.0;
const WIDE_BEAM_PIER_INTERVAL_M: f64 = 12.0;

/// Arnis's `LOWER_DECK_GAP`, a block short of the stacked headroom: a deck at
/// least this far under a pier's blocks it.
const LOWER_DECK_GAP_M: f64 = STACKED_DECK_HEADROOM_M - 1.0;

/// Arnis's `place_pillar`: a deck this near the ground rests on it.
const PIER_MIN_RISE_M: f64 = 2.0;

/// A bent's columns across the deck, from its line: the middle, both edges,
/// or both and the middle, by its half width in cells.
fn bent(half: i32) -> &'static [i32] {
    match half {
        ..=2 => &[0],
        3..=4 => &[-1, 1],
        _ => &[-1, 0, 1],
    }
}

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
    let half = half_cells(width);
    let step_m = if half <= 2 { BEAM_PILLAR_INTERVAL_M } else { WIDE_BEAM_PIER_INTERVAL_M };
    let lattice = step_m * u_per_m(frame.corner[1]);
    let station = round((frame.corner[k] + near[k] * frame.cell) / lattice) * lattice;
    let t = ((station - frame.corner[k]) / frame.cell - a[k]) / d[k];
    if !(0.0..1.0).contains(&t) {
        return false;
    }
    let len = libm::hypot(d[0], d[1]);
    let dir = [d[0] / len, d[1] / len];
    let rel = [at[0] - (a[0] + d[0] * t), at[1] - (a[1] + d[1] * t)];
    let along = (rel[0] * dir[0] + rel[1] * dir[1]).abs();
    let across = rel[0] * dir[1] - rel[1] * dir[0];
    // an edge column stands a cell in from the edge
    along <= 0.5 && bent(half).iter().any(|&side| (across - f64::from(side * (half - 1).max(0))).abs() <= 0.5)
}

/// None under a deck `PIER_MIN_RISE_M` or less over the ground, on a way at
/// grade or over a deck `LOWER_DECK_GAP_M` or more under its own (Arnis
/// `place_pillar`, `support_blocked`), none where the terrain gives no surface
/// to stand on; water stops none.
pub fn pier_stands(pier: [i32; 2], cell: GroundId, column: impl IntoIterator<Item = Crossing>, ground: i32, u_per_m: f64) -> Option<[i32; 2]> {
    if Ground::of(cell).is_none_or(Ground::transport) {
        return None;
    }
    let surface = pier[1] + slab_depth(u_per_m);
    if f64::from(surface - pier[0]) <= PIER_MIN_RISE_M * u_per_m {
        return None;
    }
    let gap = round(LOWER_DECK_GAP_M * u_per_m) as i32;
    let lower = column
        .into_iter()
        .any(|o| matches!(o.occupies(ground, u_per_m), Occupies::Over { slab, .. } if slab[1] <= surface - gap));
    (!lower).then_some(pier)
}

// ---- caves: a hash pyramid ----

/// The coarsest cube a cave takes, 32 units a side: nothing coarser is
/// hollow, so every coarser fold of the caves is solid.
pub const CAVE_COARSEST: Zoom = GROUND.cell() - 5;

/// The chance a cube turns one of its eight children over, the tree's own
/// number: some 5% of the ground hollow.
const CAVE_TURN: f64 = 1.0 / 16.0;

/// Whether the cube at `zoom` holding `at`, in units, is a cave's air. A cube
/// is its parent's value unless its parent's hash turned it over, and a
/// parent turns at most one of its eight, so the majority of any cube's
/// children is the cube: the caves fold to themselves at every zoom. Past
/// the leaves a cube is its leaf's.
pub fn hollow_at(zoom: Zoom, at: [f64; 3]) -> bool {
    (CAVE_COARSEST..=zoom.min(GROUND.cell())).fold(false, |hollow, z| hollow ^ turned(z, at.map(|v| (v / units(z)).floor() as i64)))
}

/// Whether the cube `c` at `z` is turned over from its parent.
fn turned(z: Zoom, c: [i64; 3]) -> bool {
    let seed = derive_seed_u64(MASTER_SEED, 0xca7e_0001);
    let slot = (c[0] & 1) | (c[1] & 1) << 1 | (c[2] & 1) << 2;
    let h = [u64::from(z), (c[0] >> 1) as u64, (c[1] >> 1) as u64, (c[2] >> 1) as u64].into_iter().fold(seed, derive_seed_u64);
    u32_to_unit(h as u32) < CAVE_TURN && (h >> 32) % 8 == slot as u64
}

const CAVE_LEVELS: usize = (GROUND.cell() - CAVE_COARSEST) as usize + 1;

/// The caves down one column `side` units wide, its center `x`, `y` in
/// units: its cubes are the column's own, or past the leaves its leaf's.
/// Each cube's ancestors are kept as it is asked down the column, so a
/// cube asks only the zooms whose cube it left.
pub struct Caves {
    at: [f64; 2],
    zoom: Zoom,
    chain: [Option<(i64, bool)>; CAVE_LEVELS],
}

impl Caves {
    pub fn column(x: f64, y: f64, side: f64) -> Caves {
        Caves {
            at: [x, y],
            zoom: of_side(side).min(GROUND.cell()),
            chain: [None; CAVE_LEVELS],
        }
    }

    /// Whether the cell centered at `z`, in units, is a cave's air.
    pub fn hollow(&mut self, z: f64) -> bool {
        let mut hollow = false;
        for (l, zoom) in (CAVE_COARSEST..=self.zoom).enumerate() {
            let side = units(zoom);
            let c = [self.at[0], self.at[1], z].map(|v| (v / side).floor() as i64);
            hollow = match self.chain[l] {
                Some((k, h)) if k == c[2] => h,
                _ => hollow ^ turned(zoom, c),
            };
            self.chain[l] = Some((c[2], hollow));
        }
        hollow
    }

    /// The cube `z`, in units, is in: where it ends, and whether it is
    /// hollow, as every cell in it is.
    pub fn stretch(&mut self, z: f64) -> (f64, bool) {
        let side = units(self.zoom);
        (((z / side).floor() + 1.0) * side, self.hollow(z))
    }
}

/// Whether the leaf cell centered at `at`, in units, is a cave's air.
pub fn hollow(at: [f64; 3]) -> bool {
    hollow_at(GROUND.cell(), at)
}

#[cfg(test)]
#[path = "procedural.test.rs"]
mod test;
