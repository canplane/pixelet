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
use crate::space::camera::grid::CHUNK_STEPS;
use crate::space::coord::aabb::Aabb;
use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::pvf3::ivec::gather;
use crate::space::coord::world::REACH;
use crate::space::coord::zoom::{units, Zoom};
use crate::space::layer::lattice::{GROUND, GROUND_FOLD, TILE_CELLS};
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

// ---- caves: Minecraft 1.18's noise caves ----

/// Ken Perlin's improved noise (SIGGRAPH 2002): the fade 6t⁵ − 15t⁴ + 10t³
/// and his twelve cube-edge gradients, the permutation table a hash of the
/// lattice point instead, so it needs no table and runs on as far as the world
/// does. In about [−1, 1]; plain arithmetic, so every platform draws the same.
pub fn perlin(seed: u64, [x, y, z]: [f64; 3]) -> f64 {
    Line::new(seed, x, y).at(z)
}

/// Improved noise down one line of `z` at a fixed `x`, `y`: the gradients of
/// the lattice cube it is in are hashed once, and again only where `z` leaves
/// it, which a cell does once in many.
struct Line {
    seed: u64,
    corner: [i64; 2],
    fraction: [f64; 2],
    cube: Option<(i64, [u64; 8])>,
}

impl Line {
    fn new(seed: u64, x: f64, y: f64) -> Line {
        let (fx, fy) = (x.floor(), y.floor());
        Line {
            seed,
            corner: [fx as i64, fy as i64],
            fraction: [x - fx, y - fy],
            cube: None,
        }
    }

    fn at(&mut self, z: f64) -> f64 {
        let fz = z.floor();
        let iz = fz as i64;
        let ([ix, iy], seed) = (self.corner, self.seed);
        let hashes = match self.cube {
            Some((at, h)) if at == iz => h,
            _ => {
                let h = core::array::from_fn(|k| {
                    let [dx, dy, dz] = [k & 1, (k >> 1) & 1, k >> 2].map(|d| d as i64);
                    derive_seed_u64(derive_seed_u64(derive_seed_u64(seed, (ix + dx) as u64), (iy + dy) as u64), (iz + dz) as u64) & 15
                });
                self.cube = Some((iz, h));
                h
            }
        };
        let ([x, y], z) = (self.fraction, z - fz);
        let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
        let grad = |k: usize| {
            let h = hashes[k];
            let (x, y, z) = (x - (k & 1) as f64, y - ((k >> 1) & 1) as f64, z - (k >> 2) as f64);
            let u = if h < 8 { x } else { y };
            let v = if h < 4 {
                y
            } else if h == 12 || h == 14 {
                x
            } else {
                z
            };
            (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
        };
        let (u, v, w) = (fade(x), fade(y), fade(z));
        let face = |dz: usize| lerp(v, lerp(u, grad(4 * dz), grad(4 * dz + 1)), lerp(u, grad(4 * dz + 2), grad(4 * dz + 3)));
        lerp(w, face(0), face(1))
    }
}

/// A cave's tunnels where two noises are both near zero, Minecraft's
/// spaghetti; its caverns where one is high, its cheese. The scales are a
/// unit to a block, and these numbers are the tree's own, none of Minecraft's
/// copied: a tunnel some 2 to 4 units across, a cavern rare.
const SPAGHETTI_SCALE: [f64; 3] = [1.0 / 24.0, 1.0 / 24.0, 1.0 / 12.0];
const SPAGHETTI_WIDTH: f64 = 0.06;
const CHEESE_SCALE: f64 = 1.0 / 48.0;
const CHEESE_THRESHOLD: f64 = 0.42;

/// Units between the heights a cave's noise is sampled at, and lerped
/// between, as Minecraft's `NoiseChunk` samples its cells' corners and
/// interpolates inside them. The tree's own number: a quarter of the
/// noises' cost, finer than a tunnel's span.
const KNOT: f64 = 4.0;

/// One cave noise down a column, sampled at every `KNOT` and lerped between:
/// the two knots about the last height asked are kept.
struct Knotted {
    line: Line,
    scale: f64,
    shift: f64,
    knots: Option<(i64, f64, f64)>,
}

impl Knotted {
    /// The noise at the knots under and over the knot interval `k`.
    fn ends(&mut self, k: f64) -> (f64, f64) {
        let (at, kn) = (k as i64, |s: &mut Knotted, k: f64| s.line.at(k * KNOT * s.scale + s.shift));
        let (lo, hi) = match self.knots {
            Some((a, lo, hi)) if a == at => (lo, hi),
            Some((a, _, hi)) if a + 1 == at => (hi, kn(self, k + 1.0)),
            _ => (kn(self, k), kn(self, k + 1.0)),
        };
        self.knots = Some((at, lo, hi));
        (lo, hi)
    }

    fn at(&mut self, z: f64) -> f64 {
        let t = z / KNOT;
        let k = t.floor();
        let (lo, hi) = self.ends(k);
        lo + (t - k) * (hi - lo)
    }
}

/// The caves down one column, its `x`, `y` in units: `hollow` a cell at a
/// time, each noise sampled at its knots (`KNOT`).
pub struct Caves {
    noises: [Knotted; 3],
}

impl Caves {
    pub fn column(x: f64, y: f64) -> Caves {
        // Each noise on a lattice of its own, shifted by its seed: Perlin noise is
        // zero on its lattice's points, so two on one lattice would both be near
        // zero there and hollow a grid of pits, not tunnels.
        let noise = |tag: u64, s: [f64; 3]| {
            let seed = derive_seed_u64(MASTER_SEED, tag);
            let shift = |k: u64| u32_to_unit(derive_seed_u64(seed, k) as u32);
            Knotted {
                line: Line::new(seed, x * s[0] + shift(0), y * s[1] + shift(1)),
                scale: s[2],
                shift: shift(2),
                knots: None,
            }
        };
        Caves {
            noises: [noise(0xca7e_0001, SPAGHETTI_SCALE), noise(0xca7e_0002, SPAGHETTI_SCALE), noise(0xca7e_0003, [CHEESE_SCALE; 3])],
        }
    }

    /// Whether the cell centered at `z`, in units, is a cave's air. Nothing
    /// bounds it: whatever is laid over it later, bedrock and ways among them, wins.
    pub fn hollow(&mut self, z: f64) -> bool {
        let [a, b, c] = &mut self.noises;
        // The second only where the first is near zero, a few cells in a hundred.
        if a.at(z).abs() < SPAGHETTI_WIDTH && b.at(z).abs() < SPAGHETTI_WIDTH {
            return true;
        }
        c.at(z) > CHEESE_THRESHOLD
    }

    /// The knot interval `z`, in units, is in: where it ends, and whether every
    /// cell in it is solid, every one hollow, or neither is sure. Between two
    /// knots each noise is a line, which lies between its ends, so a
    /// threshold neither end crosses no cell between them crosses either,
    /// and a cell is asked alone only where one may.
    pub fn stretch(&mut self, z: f64) -> (f64, Option<bool>) {
        // Past lerp's rounding: an end this near a threshold decides nothing.
        const SURE: f64 = 1e-9;
        let k = (z / KNOT).floor();
        let [a, b, c] = &mut self.noises;
        let beyond = |(lo, hi): (f64, f64), at: f64| (lo > at + SURE && hi > at + SURE, lo < at - SURE && hi < at - SURE);
        let cheese = beyond(c.ends(k), CHEESE_THRESHOLD);
        let narrow = |(lo, hi): (f64, f64)| {
            let (over, _) = beyond((lo, hi), SPAGHETTI_WIDTH);
            let (_, under) = beyond((lo, hi), -SPAGHETTI_WIDTH);
            over || under
        };
        let sure = match cheese {
            (true, _) => Some(true),
            (_, true) if narrow(a.ends(k)) || narrow(b.ends(k)) => Some(false),
            _ => None,
        };
        ((k + 1.0) * KNOT, sure)
    }
}

/// The zoom of the cubes that open caves: a leaf chunk's, 32 units. A cave
/// is carved only about an edit, in its cube or one beside: elsewhere it is
/// shut in rock and drawn as rock, so it costs nothing.
pub const CAVE_REGION: Zoom = GROUND.cell() - CHUNK_STEPS;

/// The coarsest grid cells a chunk carves caves in: those composed of the
/// leaves' cells, a fold one finer (`GROUND_FOLD`), as control composes.
const CAVE_CELL: Zoom = GROUND.cell() - GROUND_FOLD;

/// Whether a chunk of grid cells at `cell` carves caves, so control meets
/// the caves the chunks drawn finer show.
pub const fn caves_in(cell: Zoom) -> bool {
    cell >= CAVE_CELL
}

/// Cube columns a side the widest chunk carving caves spans.
const CHUNK_CUBES: usize = 1 << (CAVE_REGION - (CAVE_CELL - CHUNK_STEPS));

/// Cube columns a side a `CaveOpening` reads from its center, a chunk's
/// first: the chunk's, the apron's one before and after, and one beside each.
const OPENING_SIDE: usize = CHUNK_CUBES + 4;

/// The heights caves open over about edits at `[lo, hi]`, in units: the
/// cubes holding them and one more each way.
pub fn opened_heights([lo, hi]: [f64; 2]) -> [f64; 2] {
    let r = units(CAVE_REGION);
    [(lo / r).floor() * r - r, (hi / r).ceil() * r + r]
}

/// Where caves open about a column of cubes (`CAVE_REGION`) and the cube
/// columns about it: each one's edits' heights, none where nothing is
/// placed. A cave is a function of its cube alone, never of who asks, so
/// drawing and control carve the same.
#[derive(Clone, Copy)]
pub struct CaveOpening {
    first: [i64; 2],
    edits: [Option<[f64; 2]>; OPENING_SIDE * OPENING_SIDE],
}

/// Where an edit in `edit` may open or shut caves: the cube columns beside
/// its own, at every height, since a column opens over all its edits' heights.
pub fn opened_by(edit: Aabb) -> Aabb {
    let r = units(CAVE_REGION);
    let mut b = REACH;
    for a in 0..2 {
        (b.min[a], b.max[a]) = ((edit.min[a] / r).floor() * r - r, (edit.max[a] / r).ceil() * r + r);
    }
    b
}

/// Nowhere open: nothing placed about it.
impl Default for CaveOpening {
    fn default() -> CaveOpening {
        CaveOpening {
            first: [0; 2],
            edits: [None; OPENING_SIDE * OPENING_SIDE],
        }
    }
}

impl CaveOpening {
    /// The cube columns read about the cube column `center`, first and last.
    pub fn reach(center: [i64; 2]) -> [[i64; 2]; 2] {
        [center.map(|v| v - 2), center.map(|v| v - 2 + OPENING_SIDE as i64 - 1)]
    }

    /// For the cube column `center`, its edits read by `edits` for each cube
    /// column in its reach.
    pub fn about(center: [i64; 2], mut edits: impl FnMut([i64; 2]) -> Option<[f64; 2]>) -> CaveOpening {
        let [first, _] = CaveOpening::reach(center);
        let n = OPENING_SIDE;
        CaveOpening {
            first,
            edits: core::array::from_fn(|k| edits([first[0] + (k % n) as i64, first[1] + (k / n) as i64])),
        }
    }

    /// The heights caves open over at `x`, `y`, in units: the cubes about
    /// the edits in its cube column and the ones beside.
    pub fn over(&self, x: f64, y: f64) -> Option<[f64; 2]> {
        let r = units(CAVE_REGION);
        let g = [x, y].map(|v| (v / r).floor() as i64);
        let mut hull: Option<[f64; 2]> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let ([ix, iy], n) = ([g[0] + dx - self.first[0], g[1] + dy - self.first[1]], OPENING_SIDE as i64);
                let Some(z) = ((0..n).contains(&ix) && (0..n).contains(&iy)).then(|| self.edits[(iy * n + ix) as usize]).flatten() else {
                    continue;
                };
                hull = Some(hull.map_or(z, |h| [h[0].min(z[0]), h[1].max(z[1])]));
            }
        }
        hull.map(opened_heights)
    }
}

/// Whether the cell centered at `at`, in units, is a cave's air (`Caves`).
pub fn hollow(at: [f64; 3]) -> bool {
    Caves::column(at[0], at[1]).hollow(at[2])
}

#[cfg(test)]
#[path = "procedural.test.rs"]
mod test;
