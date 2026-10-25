// pxl-geo-bake/separation/arnis.rs

// Copyright (c) 2022-2026 Louis Erbkamm (louis-e)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! louis-e/arnis `c96872e` ported, changed to read Overture's fields for
//! OSM's tags and to take its widths from the ground: bridges and tunnels
//! for one tile, heights as Arnis solves them in its element processing for
//! bridges, highways and railways, without the blocks it places: a road
//! bridge's deck is the
//! least concave line over what it must clear, a rail viaduct one level with
//! ramps, a road tunnel as deep as its cover asks and reached by its approach
//! roads sinking into the portal, a rail tunnel at a fixed depth under the
//! terrain. Arnis's numbers are metres (a block a metre upright on the Earth,
//! as its celestial bodies say) taken into the tile's cells, half to even.
//! What is read is the tile and as much of its 1-ring as the structures
//! touching it reach, and no further.
//!
//! The 1-ring is the most a tile may consult, not what it must: nothing is
//! read past it, and nothing reads all of it where less will do. A tile draws
//! only itself (`terrain/pipeline/separation.rs` keeps what lies in it), so
//! what it computes past what bears on its own cells is spent for nothing. Two
//! rules follow, and a change here keeps both:
//!
//! - Where the ring cuts a piece is no end of it. A cut lies a tile from every
//!   cell its tile draws, so a height that leans on nothing further than a
//!   tile comes out the same in two neighbours, who both see the cells where
//!   they meet; a height drawn from the cut, or from anything only the whole
//!   piece shows (its lowest point, the straight run between its portals),
//!   comes out different, and their tiles step at the seam.
//! - What only the whole piece can answer is not answered by reading past the
//!   ring for it, which costs every tile reads it is not owed. A tile that
//!   cannot see it does without it, by a rule that leans on what it sees
//!   (a long bore, `road_tunnels`; a deck, `deck_block`).
//!
//! Arnis is where this solve started, not a bound on it: where its answer
//! needs a structure whole, the ring wins and the answer is reworked.

use fnv::{FnvHashMap, FnvHashSet};
use pxl_spec::procedural::Frame;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::terrain::CELL_U;
use pxl_spec::bake::separation::{footprint as line_reach, Form, STACKED_DECK_HEADROOM_M};
use pxl_spec::space::coord::adapter::epsg3857::{get_tile_bbox, mer_from_u, u_from_mer};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec_core::ways;
use xpute_kit::math::scalar::round;

use crate::segment::{End, PartId, Segment};
use crate::separation::elevation::{sample_cells, tile_of};
use crate::separation::piece::{piece_of, Piece};
use crate::separation::source::{portals_in, ring, Store};
use crate::separation::way::{Polyline, Way};
use crate::separation::{Drawn, Line, Reader, Solved};
use crate::terrain::pipeline::overture::policy::waterway_width;
use crate::terrain::pipeline::overture::width::width_u;
use crate::wkb::wkb_to_geometries;

// ---- Arnis's numbers, in metres or as ratios, under its names ----
const LAYER_HEIGHT_STEP_M: f64 = 6.0;
const FLAT_TERRAIN_DIP_THRESHOLD_M: f64 = 4.0;
const SHORT_BRIDGE_LENGTH_M: f64 = 30.0;
const DUAL_CARRIAGEWAY_MAX_DISTANCE_M: f64 = 12.0;
const DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG: f64 = 20.0;
const SIDE_DECK_EDGE_GAP_M: f64 = 2.0;
const ROAD_HEADROOM_M: f64 = 6.0;
const PATH_HEADROOM_M: f64 = 5.0;
const RAIL_HEADROOM_M: f64 = 8.0;
const RIVER_HEADROOM_M: f64 = 4.0;
const STREAM_HEADROOM_M: f64 = 2.0;
const MAX_CLEARANCE_SLOPE: f64 = 1.0;
const PROFILE_MONOTONIC_TOLERANCE_M: f64 = 2.0;
const OBSTACLE_GRID_M: f64 = 64.0;
const RAIL_ON_DECK_REACH_M: f64 = 2.0;
const DECK_RAMP_FRACTION: f64 = 0.35;
const DECK_RAMP_M: [f64; 2] = [15.0, 50.0];
const RAIL_BRIDGE_FLAT_CLEARANCE_M: f64 = 4.0;
const RAIL_BRIDGE_DIP_THRESHOLD_M: f64 = 4.0;
const RAIL_BRIDGE_RAMP_M: [f64; 2] = [8.0, 30.0];
const RAIL_BRIDGE_RAMP_FRACTION: f64 = 0.25;
const TUNNEL_CEIL_OFFSET_M: f64 = 5.0;
const TUNNEL_COVER_DROP_M: f64 = 7.0;
const TUNNEL_ROOF_COVER_M: f64 = TUNNEL_CEIL_OFFSET_M + 1.0;
const TUNNEL_RAMP_STEP: i32 = 1;
const TUNNEL_RAMP_RUN: i32 = 3;
const TUNNEL_LAYER_DROP_M: f64 = 7.0;
const TUNNEL_MAX_LAYERS_BELOW: i64 = 8;
const TUNNEL_APPROACH_WAYS: usize = 8;
/// Ours, not Arnis's: the steepest a bore longer than a tile rises from the lowest floor near it
/// (`road_tunnels`): 2%, which a road tunnel is built within.
const TUNNEL_FAR_GRADE: f64 = 0.02;
const RAIL_TUNNEL_DEPTH_M: f64 = 3.0;
const RAIL_INTERIOR_HEIGHT_M: f64 = 4.0;
/// A block, as a length: what Arnis adds or takes by one.
const BLOCK_M: f64 = 1.0;

/// The tile's scale: Arnis's metres in leaf cells (pxl-spec-core
/// `cells_of_m`), at the tile's `Frame::per_m`.
#[derive(Clone, Copy)]
struct Units {
    per_m: f32,
}

impl Units {
    fn c(self, m: f64) -> i32 {
        pxl_spec_core::cells_of_m(m as f32, self.per_m)
    }
}

/// What the search has read: the ring's tiles it reached, their ground, every
/// way on them, their water, and their structures' pieces.
struct Source {
    ring: BTreeSet<[i32; 2]>,
    loaded: BTreeSet<[i32; 2]>,
    ground: FnvHashMap<[i32; 2], Vec<i16>>,
    ways: BTreeMap<PartId, Way>,
    water: Vec<(Option<String>, String, Vec<[f64; 2]>)>,
    water_seen: FnvHashSet<Vec<u8>>,
    pieces: BTreeMap<PartId, Sampled>,
    by_end: FnvHashMap<End, Vec<PartId>>,
    /// Every way by its two ends.
    way_ends: FnvHashMap<End, Vec<PartId>>,
    /// The pieces read since the scope last looked.
    fresh: Vec<PartId>,
}

/// A piece, and once wanted its samples a cell apart along its way inside the ring.
struct Sampled {
    piece: Piece,
    n: usize,
    samples: Option<BTreeMap<usize, ([f64; 2], i32)>>,
}

/// Whether a piece is a road tunnel: one Arnis bores as deep as its cover
/// asks, where a rail tunnel keeps a depth under the ground.
fn road_tunnel(piece: &Piece) -> bool {
    piece.form == Form::Tunnel && piece.way.subtype != "rail"
}

/// A stretch of a piece inside the ring: what Arnis takes for a way, its
/// cells a cell apart.
struct Run {
    piece: Piece,
    k0: usize,
    /// Cells: DB coordinates, the ring's cell coordinates, the terrain.
    at: Vec<[f64; 2]>,
    cell: Vec<[f64; 2]>,
    terrain: Vec<i32>,
    /// The piece's line whole, in cells from the world's corner (`absolute`):
    /// its geometry is every tile's alike, wherever its ring cuts it, though
    /// its ground is not.
    whole: Polyline,
    ends: [End; 2],
    /// Whether an end is where the ring cut the piece.
    clipped: [bool; 2],
    /// Every node it passes, ends included.
    nodes: Vec<End>,
    half: i32,
    width_u: f64,
}

impl Run {
    /// A railway's, not a road's: Arnis solves the two apart.
    fn rail(&self) -> bool {
        self.piece.way.subtype == "rail"
    }
}

/// A way at grade made to stand off it, a bridge's ramp or a tunnel's
/// approach: a grade line at `points`, which ends where the way does at
/// either end `ends` says (`coarsest`), drawing `range` of its segment.
fn graded(way: &Way, points: Vec<[f64; 3]>, ends: [bool; 2], range: [f64; 2]) -> (Line, Drawn) {
    (
        Line {
            subtype: way.row.subtype.clone(),
            class: way.row.class.clone(),
            width_u: way_width(&way.row),
            form: Form::Approach,
            coarsest: super::coarsest(&points, ends),
            points,
            clear: 0,
        },
        Drawn {
            gers_id: way.row.gers_id.clone(),
            range,
        },
    )
}

pub fn solve_tile(r: &mut Reader<impl Store>, tile: [i32; 2]) -> Result<Solved, String> {
    let tiles = ring(tile);
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for &p in &tiles {
        for c in get_tile_bbox(LEAF_ZOOM, p) {
            for k in 0..2 {
                (lo[k], hi[k]) = (lo[k].min(c[k]), hi[k].max(c[k]));
            }
        }
    }
    let [pmin, pmax] = get_tile_bbox(LEAF_ZOOM, tile);
    let units = Units {
        per_m: Frame::of(Tile::new(LEAF_ZOOM, tile)).per_m(),
    };
    let mut src = Source {
        ring: tiles.into_iter().collect(),
        loaded: BTreeSet::new(),
        ground: FnvHashMap::default(),
        ways: BTreeMap::new(),
        water: Vec::new(),
        water_seen: FnvHashSet::default(),
        pieces: BTreeMap::new(),
        by_end: FnvHashMap::default(),
        way_ends: FnvHashMap::default(),
        fresh: Vec::new(),
    };
    src.load(r, tile)?;

    // ---- the structures the tile holds and every one they reach: shared ends, decks
    // beside or across them; each read as far as the ring ----
    let own = [[pmin[0], pmin[1]], [pmax[0], pmax[1]]];
    // a road tunnel whose portal lies within its approaches' reach of the tile sinks ways the tile
    // draws, whether or not it touches the tile: the tunnels about the tile are asked for alone, as far
    // as the deepest one's approaches run, a reach well inside the ring, and only about a portal that
    // near are the ring's tiles read, as far as its approaches run
    let reach_of = |level: i64| mer_from_u(f64::from(tunnel_drop(level, units) * TUNNEL_RAMP_RUN) * CELL_U);
    let off = |p: [f64; 2]| {
        let d = [0, 1].map(|k| (own[0][k] - p[k]).max(p[k] - own[1][k]).max(0.0));
        libm::hypot(d[0], d[1])
    };
    for p in portals_in(r.client, grown(own, reach_of(-(TUNNEL_MAX_LAYERS_BELOW + 1))))? {
        let d = reach_of(p.level);
        if off(p.at) <= d {
            let [a, b] = [tile_of([p.at[0] - d, p.at[1] - d]), tile_of([p.at[0] + d, p.at[1] + d])];
            for y in a[1].min(b[1])..=a[1].max(b[1]) {
                for x in a[0].min(b[0])..=a[0].max(b[0]) {
                    src.load(r, [x, y])?;
                }
            }
        }
    }
    let portal_near = |s: &Sampled| {
        let p = &s.piece.way.points;
        road_tunnel(&s.piece) && [p[0], p[p.len() - 1]].iter().any(|e| off([e[0], e[1]]) <= reach_of(s.piece.way.tags.level.unwrap_or(0)))
    };
    let mut queue: Vec<PartId> = src.pieces.iter().filter(|(_, s)| s.piece.touches(own) || portal_near(s)).map(|(id, _)| id.clone()).collect();
    // a tile no structure touches nor sinks into has nothing to solve
    if queue.is_empty() {
        return Ok(Solved::default());
    }
    let reach = mer_from_u(f64::from(units.c(DUAL_CARRIAGEWAY_MAX_DISTANCE_M) + 16) * CELL_U);
    // each piece is looked at once, against the pieces read so far; a piece a later read brings in
    // is held against what was looked at before it (`seen`), so no pass is repeated
    let lies_on = |o: &Piece, ends: &FnvHashSet<End>| (o.form == Form::Bridge || road_tunnel(o)) && o.way.points.iter().any(|p| ends.contains(&End::at([p[0], p[1]])));
    let mut scope: BTreeSet<PartId> = BTreeSet::new();
    let (mut seen_ends, mut seen_portals, mut seen_decks): (FnvHashSet<End>, FnvHashSet<End>, Vec<[[f64; 2]; 2]>) = Default::default();
    src.fresh.clear();
    while let Some(id) = queue.pop() {
        if !scope.insert(id.clone()) {
            continue;
        }
        src.sample(r, &id)?;
        let s = &src.pieces[&id];
        let mut reached: Vec<PartId> = s.piece.way.ends.iter().flat_map(|e| src.by_end.get(e).into_iter().flatten().cloned()).collect();
        seen_ends.extend(s.piece.way.ends.iter().cloned());
        if road_tunnel(&s.piece) {
            // what an end of it lies on, at that one's end or along it: a deck its portal opens
            // on, or a road tunnel it runs on in
            let ends: FnvHashSet<End> = s.piece.way.ends.iter().cloned().collect();
            reached.extend(src.pieces.iter().filter(|(_, o)| lies_on(&o.piece, &ends)).map(|(id, _)| id.clone()));
            seen_portals.extend(ends);
        }
        if s.piece.form == Form::Bridge {
            let b = grown(bbox_of(s), reach);
            reached.extend(src.pieces.iter().filter(|(_, o)| o.piece.form == Form::Bridge && o.piece.touches(b)).map(|(id, _)| id.clone()));
            seen_decks.push(b);
        }
        // a tile read late can hold what one looked at early meets or stands beside
        for f in std::mem::take(&mut src.fresh) {
            let o = &src.pieces[&f].piece;
            if o.way.ends.iter().any(|e| seen_ends.contains(e)) || (o.form == Form::Bridge && seen_decks.iter().any(|&b| o.touches(b))) || lies_on(o, &seen_portals) {
                reached.push(f);
            }
        }
        queue.extend(reached.into_iter().filter(|q| !scope.contains(q)));
    }
    let runs = runs_of(&src, &scope, lo);
    // what a deck must clear, only where there is a deck
    let obstacles = if runs.iter().any(|r| r.piece.form == Form::Bridge) {
        Obstacles::of(&src, [lo, hi], units)
    } else {
        Obstacles::none()
    };
    let mut lines: Vec<(Line, Drawn)> = Vec::new();

    // ---- road bridges, lower first; rail bridges over them; then the tunnels ----
    let solved = bridges(&src, &runs, &obstacles, units, &mut lines);
    for (q, ys) in &solved.ys {
        lines.push(part_of(&runs[*q], ys, [0, ys.len() - 1], Form::Bridge, 0));
    }
    for (q, ys) in rail_bridges(&runs, &obstacles, &solved.separation, units) {
        lines.push(part_of(&runs[q], &ys, [0, ys.len() - 1], Form::Bridge, 0));
    }
    let ceiling = (units.c(TUNNEL_CEIL_OFFSET_M) - units.c(BLOCK_M)).clamp(0, 255) as u8;
    for (q, ys, covered) in road_tunnels(&src, &runs, &solved, units, &mut lines) {
        let r = &runs[q];
        // a stretch under cover keeps its ceiling, one open to the sky is cut to the surface, and
        // one a deck's portal holds over the ground is an embankment up to it
        let bore: Vec<Bore> = (0..ys.len())
            .map(|i| {
                if ys[i] > r.terrain[i] {
                    Bore::Raised
                } else if covered[i] {
                    Bore::Covered
                } else {
                    Bore::Open
                }
            })
            .collect();
        for (stretch, bore) in split_by(&bore) {
            let (form, clear) = match bore {
                Bore::Covered => (Form::Tunnel, ceiling),
                Bore::Open => (Form::Tunnel, u8::MAX),
                Bore::Raised => (Form::Approach, 0),
            };
            lines.push(part_of(r, &ys, stretch, form, clear));
        }
    }
    for q in (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Tunnel && runs[q].rail()) {
        // the floor under the interior, under the ceiling, under the cover (Arnis `generate_rail_tunnel_shell`)
        let depth = units.c(RAIL_TUNNEL_DEPTH_M + RAIL_INTERIOR_HEIGHT_M + BLOCK_M);
        let ys: Vec<i32> = runs[q].terrain.iter().map(|t| t - depth).collect();
        lines.push(part_of(&runs[q], &ys, [0, ys.len() - 1], Form::Tunnel, units.c(RAIL_INTERIOR_HEIGHT_M).clamp(0, 255) as u8));
    }
    let mut out = Solved::default();
    for (line, drawn) in lines.into_iter().filter(|(l, _)| l.points.len() >= 2) {
        out.lines.push(line);
        out.drawn.push(drawn);
    }
    Ok(out)
}

/// Of a piece's samples, its cells in the ring.
fn bbox_of(s: &Sampled) -> [[f64; 2]; 2] {
    let mut b = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    for (q, _) in s.samples.iter().flat_map(|m| m.values()) {
        for k in 0..2 {
            (b[0][k], b[1][k]) = (b[0][k].min(q[k]), b[1][k].max(q[k]));
        }
    }
    b
}

fn grown(b: [[f64; 2]; 2], by: f64) -> [[f64; 2]; 2] {
    [[b[0][0] - by, b[0][1] - by], [b[1][0] + by, b[1][1] + by]]
}

impl Source {
    /// Whether a point lies in the ring: past it nothing of a way is used.
    fn inside(&self, p: [f64; 2]) -> bool {
        self.ring.contains(&tile_of(p))
    }

    /// Whether a point's ground is held: its tile of the ring was read.
    fn held(&self, p: [f64; 2]) -> bool {
        self.loaded.contains(&tile_of(p))
    }

    /// Reads a tile of the ring once; a tile past it is never read.
    fn load(&mut self, r: &mut Reader<impl Store>, p: [i32; 2]) -> Result<(), String> {
        if !self.ring.contains(&p) || self.loaded.contains(&p) {
            return Ok(());
        }
        self.loaded.insert(p);
        let rows = r.store.tile(r.client, r.repo, p)?;
        self.ground.insert(p, rows.ele().to_vec());
        let step = mer_from_u(CELL_U);
        for s in rows.ways() {
            let key = s.id();
            if self.ways.contains_key(&key) || s.subtype == "water" {
                continue;
            }
            if let Some(piece) = piece_of(s) {
                let id = piece.id();
                for e in &piece.way.ends {
                    self.by_end.entry(e.clone()).or_default().push(id.clone());
                }
                let n = libm::ceil(piece.way.length() / step).max(1.0) as usize;
                self.fresh.push(id.clone());
                self.pieces.insert(id, Sampled { piece, n, samples: None });
            }
            if let Some(way) = Way::of(s.clone()) {
                for e in &s.ends {
                    self.way_ends.entry(e.clone()).or_default().push(key.clone());
                }
                self.ways.insert(key, way);
            }
        }
        for (subtype, class, wkb) in rows.water() {
            if !self.water_seen.insert(wkb.clone()) {
                continue;
            }
            for g in wkb_to_geometries(wkb).into_iter().filter(|g| g.points.len() >= 2) {
                self.water.push((class.clone(), subtype.clone(), g.points.iter().map(|p| [p[0], p[1]]).collect()));
            }
        }
        Ok(())
    }

    /// A piece's cells inside the ring, the tiles its footprint reaches read first.
    fn sample(&mut self, r: &mut Reader<impl Store>, id: &PartId) -> Result<(), String> {
        let s = &self.pieces[id];
        if s.samples.is_some() {
            return Ok(());
        }
        let step = mer_from_u(CELL_U);
        let line = Polyline::of(s.piece.way.points.iter().map(|p| [p[0], p[1]]));
        let length = s.piece.way.length();
        let reach = mer_from_u((line_reach(s.piece.width() / CELL_U) + 16.0) * CELL_U);
        let at: Vec<(usize, [f64; 2])> = (0..=s.n)
            .map(|k| (k, line.at((k as f64 * step).min(length))))
            .filter(|(_, p)| self.ring.contains(&tile_of(*p)))
            .collect();
        let mut wanted: BTreeSet<[i32; 2]> = BTreeSet::new();
        for (_, p) in &at {
            for d in [[0.0, 0.0], [reach, 0.0], [-reach, 0.0], [0.0, reach], [0.0, -reach]] {
                wanted.insert(tile_of([p[0] + d[0], p[1] + d[1]]));
            }
        }
        for e in [s.piece.way.points[0], s.piece.way.points[s.piece.way.points.len() - 1]] {
            wanted.insert(tile_of([e[0], e[1]]));
        }
        for p in wanted {
            self.load(r, p)?;
        }
        let ts = sample_cells(&self.ground, &at.iter().map(|a| a.1).collect::<Vec<_>>())?;
        self.pieces.get_mut(id).expect("a piece read").samples = Some(at.into_iter().zip(ts).map(|((k, p), t)| (k, (p, t))).collect());
        Ok(())
    }
}

/// Each piece in scope as its stretches inside the ring.
fn runs_of(src: &Source, scope: &BTreeSet<PartId>, origin: [f64; 2]) -> Vec<Run> {
    let mut out = Vec::new();
    for id in scope {
        let s = &src.pieces[id];
        let Some(samples) = &s.samples else { continue };
        let ks: Vec<usize> = samples.keys().copied().collect();
        let mut i = 0;
        while i < ks.len() {
            let first = i;
            while i + 1 < ks.len() && ks[i + 1] == ks[i] + 1 {
                i += 1;
            }
            let (k0, k1) = (ks[first], ks[i]);
            i += 1;
            if k1 == k0 {
                continue;
            }
            let at: Vec<[f64; 2]> = (k0..=k1).map(|k| samples[&k].0).collect();
            let terrain: Vec<i32> = (k0..=k1).map(|k| samples[&k].1).collect();
            // a run's end the ring cut (`End::Ring`) marks where this tile stops looking, never an
            // end of the piece: nothing joins there, and no height may be drawn from it (the module's
            // rules)
            let clipped = [k0 != 0, k1 != s.n];
            let cut = |k: usize| End::Ring(id.clone(), k as u64);
            let ends = [
                if clipped[0] { cut(k0) } else { s.piece.way.ends[0].clone() },
                if clipped[1] { cut(k1) } else { s.piece.way.ends[1].clone() },
            ];
            let mut nodes: Vec<End> = s.piece.way.points.iter().map(|p| [p[0], p[1]]).filter(|&p| src.inside(p)).map(End::at).collect();
            nodes.extend(ends.iter().cloned());
            out.push(Run {
                piece: s.piece.clone(),
                k0,
                cell: at.iter().map(|p| [u_from_mer(p[0] - origin[0]) / CELL_U, u_from_mer(p[1] - origin[1]) / CELL_U]).collect(),
                whole: Polyline::of(s.piece.way.points.iter().map(|p| absolute([p[0], p[1]]))),
                at,
                terrain,
                ends,
                clipped,
                nodes,
                half: ways::half_cells((s.piece.width() / CELL_U) as f32),
                width_u: s.piece.width(),
            });
        }
    }
    out
}

fn is_non_vehicular(class: &str) -> bool {
    matches!(class, "footway" | "pedestrian" | "path" | "cycleway" | "bridleway" | "steps")
}

/// Arnis's `is_pedestrian_way_tags`: what a tunnel's approach matches on foot.
fn is_pedestrian(class: &str) -> bool {
    matches!(class, "footway" | "pedestrian" | "steps")
}

/// Arnis's `effective_layer`: the layer, never below 0; a bridge untagged is 1.
fn effective_layer(r: &Run) -> i64 {
    r.piece.way.tags.level.map(|l| l.max(0)).unwrap_or(i64::from(r.piece.form == Form::Bridge))
}

/// The cells about one whose centers lie within `reach` cells of its own, as
/// a way covers them (pxl-spec-core `covers`, here its distance to its own).
fn within(reach: f32) -> Vec<[i64; 2]> {
    let r = libm::ceilf(reach) as i64;
    (-r..=r)
        .flat_map(|dz| (-r..=r).map(move |dx| [dx, dz]))
        .filter(|&[dx, dz]| libm::hypotf(dx as f32, dz as f32) <= reach)
        .collect()
}

/// A way's width, in u.
fn way_width(row: &Segment) -> f64 {
    width_u(&row.subtype, row.class.as_deref(), row.tags.role)
}

// ---- what a deck must clear: ways at grade and waterways, bucketed ----

struct Obstacle {
    a: [f64; 2],
    b: [f64; 2],
    /// In cells: what it covers is as the client's way covers (pxl-spec-core `covers`).
    width: f32,
    headroom: i32,
    nodes: Vec<End>,
}

struct Obstacles {
    list: Vec<Obstacle>,
    buckets: FnvHashMap<(i64, i64), Vec<usize>>,
    bucket: f64,
}

impl Obstacles {
    /// Each segment cut to the ring `[origin, far]`, its nodes those in it.
    /// Nothing to clear: where no deck is solved.
    fn none() -> Obstacles {
        Obstacles {
            list: Vec::new(),
            buckets: FnvHashMap::default(),
            bucket: 1.0,
        }
    }

    fn of(src: &Source, [origin, far]: [[f64; 2]; 2], units: Units) -> Obstacles {
        let cell = |p: [f64; 2]| [u_from_mer(p[0] - origin[0]) / CELL_U, u_from_mer(p[1] - origin[1]) / CELL_U];
        let mut out = Obstacles {
            list: Vec::new(),
            buckets: FnvHashMap::default(),
            bucket: f64::from(units.c(OBSTACLE_GRID_M).max(1)),
        };
        let add = |out: &mut Obstacles, points: &[[f64; 2]], ends: &[End], width: f32, headroom: i32| {
            let at_end = [points[0], points[points.len() - 1]];
            let nodes: Vec<End> = points
                .iter()
                .filter(|&&p| src.inside(p))
                .map(|&p| End::at(p))
                .chain(ends.iter().zip(at_end).filter(|(_, p)| src.inside(*p)).map(|(e, _)| e.clone()))
                .collect();
            for pair in points.windows(2) {
                let Some((a, b)) = clip(pair[0], pair[1], [origin, far]) else { continue };
                let pair = [cell(a), cell(b)];
                let i = out.list.len();
                out.list.push(Obstacle {
                    a: pair[0],
                    b: pair[1],
                    width,
                    headroom,
                    nodes: nodes.clone(),
                });
                let r = f64::from(ways::reach(width));
                let k = |v: f64| (v / out.bucket).floor() as i64;
                for bx in k(pair[0][0].min(pair[1][0]) - r)..=k(pair[0][0].max(pair[1][0]) + r) {
                    for by in k(pair[0][1].min(pair[1][1]) - r)..=k(pair[0][1].max(pair[1][1]) + r) {
                        out.buckets.entry((bx, by)).or_default().push(i);
                    }
                }
            }
        };
        for way in src.ways.values() {
            let row = &way.row;
            // a bridge, a tunnel or a way indoors is no grade a deck clears (Arnis `grade_obstacle`)
            if row.tags.bridge || row.tags.tunnel || row.tags.indoor {
                continue;
            }
            match row.subtype.as_str() {
                "road" => {
                    let class = row.class.as_deref().unwrap_or("");
                    let headroom = if is_non_vehicular(class) || class == "track" { PATH_HEADROOM_M } else { ROAD_HEADROOM_M };
                    add(&mut out, &way.line.points, &row.ends, (way_width(row) / CELL_U) as f32, units.c(headroom));
                }
                "rail" if !row.is_subway() => {
                    let headroom = if row.class.as_deref() == Some("tram") { ROAD_HEADROOM_M } else { RAIL_HEADROOM_M };
                    add(&mut out, &way.line.points, &row.ends, (way_width(row) / CELL_U) as f32, units.c(headroom));
                }
                _ => {}
            }
        }
        for (class, subtype, points) in &src.water {
            let class = class.as_deref().unwrap_or(subtype);
            let headroom = match class {
                "river" | "canal" => RIVER_HEADROOM_M,
                "stream" => STREAM_HEADROOM_M,
                _ => continue,
            };
            // as wide as the ground draws it, where Arnis clears its centerline
            add(&mut out, points, &[], (waterway_width(class).unwrap_or(0.0) / CELL_U) as f32, units.c(headroom));
        }
        out
    }

    fn at(&self, p: [f64; 2]) -> impl Iterator<Item = &Obstacle> {
        let k = ((p[0] / self.bucket).floor() as i64, (p[1] / self.bucket).floor() as i64);
        self.buckets.get(&k).into_iter().flatten().map(|&i| &self.list[i])
    }
}

/// The part of `a`–`b` inside `[lo, hi]` (Liang–Barsky).
fn clip(a: [f64; 2], b: [f64; 2], [lo, hi]: [[f64; 2]; 2]) -> Option<([f64; 2], [f64; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for k in 0..2 {
        for (p, q) in [(-d[k], a[k] - lo[k]), (d[k], hi[k] - a[k])] {
            if p == 0.0 {
                if q < 0.0 {
                    return None;
                }
            } else if p < 0.0 {
                t0 = t0.max(q / p);
            } else {
                t1 = t1.min(q / p);
            }
        }
    }
    let at = |t: f64| [a[0] + d[0] * t, a[1] + d[1] * t];
    (t0 <= t1).then(|| (at(t0), at(t1)))
}

/// False when a deck runs along the segment rather than across it (cos 30°).
fn crosses(heading: [f64; 2], o: &Obstacle) -> bool {
    let s = [o.b[0] - o.a[0], o.b[1] - o.a[1]];
    let norms = libm::hypot(heading[0], heading[1]) * libm::hypot(s[0], s[1]);
    norms <= 0.0 || (heading[0] * s[0] + heading[1] * s[1]).abs() / norms < 0.866
}

fn dist_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    libm::hypot(p[0] - a[0] - t * d[0], p[1] - a[1] - t * d[1])
}

// ---- road bridges (Arnis `BridgeStructureMap::build`) ----

/// Decks resolved: by cell, the deck's height and layer.
type Separation = FnvHashMap<(i64, i64), (i32, i64)>;

struct Bridges {
    ys: Vec<(usize, Vec<i32>)>,
    separation: Separation,
    /// The ways a bridge's ramp took, kept from a tunnel's approach.
    ramp_ways: FnvHashSet<PartId>,
}

fn cell_key(c: [f64; 2]) -> (i64, i64) {
    (c[0].floor() as i64, c[1].floor() as i64)
}

/// The way's heading end to end, whole, not as far as the ring cut it.
fn heading_of(r: &Run) -> Option<f64> {
    let p = &r.whole.points;
    let (s, e) = (p[0], p[p.len() - 1]);
    let d = [e[0] - s[0], e[1] - s[1]];
    (d != [0.0, 0.0]).then(|| libm::atan2(d[1], d[0]).to_degrees())
}

fn parallel(a: f64, b: f64) -> bool {
    let mut diff = (a - b).abs() % 360.0;
    if diff > 180.0 {
        diff = 360.0 - diff;
    }
    diff <= DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG || (180.0 - diff).abs() <= DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG
}

/// The box about cells.
fn cells_box(cells: &[[f64; 2]]) -> [[f64; 2]; 2] {
    let mut b = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    for c in cells {
        for k in 0..2 {
            (b[0][k], b[1][k]) = (b[0][k].min(c[k]), b[1][k].max(c[k]));
        }
    }
    b
}

fn lateral(p: [f64; 2], r: &Run) -> f64 {
    r.whole.points.windows(2).map(|w| dist_to_segment(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}

/// Arnis `side_by_side`: parallel decks forming one, across layers only when they touch. Weighed
/// on the ways whole, as Arnis weighs an OSM way, so two tiles that see both decide alike wherever
/// their rings cut them.
fn side_by_side(a: &Run, b: &Run, units: Units) -> bool {
    let (Some(ha), Some(hb)) = (heading_of(a), heading_of(b)) else { return false };
    if !parallel(ha, hb) {
        return false;
    }
    let same = effective_layer(a) == effective_layer(b);
    // their edges meet where each way's cover ends (pxl-spec-core `reach`)
    let reach = |r: &Run| f64::from(ways::reach((r.width_u / CELL_U) as f32));
    let touching = reach(a) + reach(b) + f64::from(units.c(SIDE_DECK_EDGE_GAP_M));
    let max = if same { touching.max(f64::from(units.c(DUAL_CARRIAGEWAY_MAX_DISTANCE_M))) } else { touching };
    let (short, long) = if a.whole.length() <= b.whole.length() { (a, b) } else { (b, a) };
    let offsets: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0].iter().map(|f| lateral(short.whole.at(f * short.whole.length()), long)).collect();
    let alongside = offsets.iter().filter(|&&d| d <= max).count();
    if same {
        alongside * 2 > offsets.len()
    } else {
        alongside == offsets.len() && offsets.iter().sum::<f64>() / offsets.len() as f64 >= 1.5
    }
}

struct Uf(Vec<usize>);

impl Uf {
    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

/// The least concave majorant: clears every requirement without sagging into a gap.
fn upper_hull(mut pts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.total_cmp(&a.1)));
    let mut hull: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for p in pts {
        if hull.last().is_some_and(|l| (l.0 - p.0).abs() < 1e-4) {
            continue;
        }
        while hull.len() >= 2 {
            let (o, a) = (hull[hull.len() - 2], hull[hull.len() - 1]);
            if (a.0 - o.0) * (p.1 - o.1) - (a.1 - o.1) * (p.0 - o.0) >= 0.0 {
                hull.pop();
            } else {
                break;
            }
        }
        hull.push(p);
    }
    hull
}

fn hull_at(hull: &[(f64, f64)], u: f64) -> f64 {
    match hull {
        [] => 0.0,
        [only] => only.1,
        [first, .., last] => {
            if u <= first.0 {
                return first.1;
            }
            if u >= last.0 {
                return last.1;
            }
            let i = hull.partition_point(|p| p.0 <= u);
            let (a, b) = (hull[i - 1], hull[i]);
            a.1 + (b.1 - a.1) * (u - a.0) / (b.0 - a.0).max(1e-6)
        }
    }
}

fn is_monotonic(us: &[f64], tolerance: f64) -> bool {
    let (Some(&first), Some(&last)) = (us.first(), us.last()) else { return true };
    let rising = last >= first;
    let mut extreme = first;
    for &u in us {
        if rising {
            if u < extreme - tolerance {
                return false;
            }
            extreme = extreme.max(u);
        } else {
            if u > extreme + tolerance {
                return false;
            }
            extreme = extreme.min(u);
        }
    }
    true
}

/// A way's line from the end it has there, onward, and its far end.
fn way_from<'a>(way: &'a Way, node: &End) -> Option<(Vec<[f64; 2]>, &'a End)> {
    let pts = &way.line.points;
    if &way.row.ends[0] == node {
        Some((pts.clone(), &way.row.ends[1]))
    } else if &way.row.ends[1] == node {
        Some((pts.iter().rev().copied().collect(), &way.row.ends[0]))
    } else {
        None
    }
}

/// Points a cell apart along a line, from its start.
fn cells_along(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let line = Polyline::of(points.iter().copied());
    let step = mer_from_u(CELL_U);
    let n = libm::ceil(line.length() / step).max(1.0) as usize;
    (0..=n).map(|k| line.at((k as f64 * step).min(line.length()))).collect()
}

fn ramp_len(total: f64, units: Units) -> f64 {
    (total * DECK_RAMP_FRACTION).clamp(f64::from(units.c(DECK_RAMP_M[0])), f64::from(units.c(DECK_RAMP_M[1])))
}

// ---- a deck, leaning on nothing further than a tile ----

/// A deck is solved in blocks of `DECK_BLOCK` cells along its way whole, each from what lies about
/// the block's middle, and a cell between two middles is the blend of the two; the blocks fall where
/// they fall on the way, wherever a ring cuts it (the module's rules). What the source gives, the
/// ground, the ways and waters a deck clears, its ends, is weighed within `DECK_WINDOW`. What another
/// deck sets, the deck it clears and the joint it meets, leans in turn on that deck's own window, so
/// it is weighed only within `DECK_NEAR`, the cell's own included, and the reach adds up over the
/// levels stacked (a deck over a deck, a track riding it, a bore opening on it): `DECK_LEVELS` of them
/// fit in a tile, which every ring holds about what its tile draws. Arnis solves a structure as one,
/// over all of it, which no ring can; a deck no longer than a window comes out much as it would.
const DECK_BLOCK: usize = 16;
const DECK_WINDOW: usize = 384;
const DECK_NEAR: usize = 16;
const DECK_LEVELS: usize = 4;
const _: () = assert!(DECK_LEVELS * DECK_BLOCK + (DECK_LEVELS - 1) * DECK_NEAR + DECK_WINDOW <= TILE_CELLS as usize);

/// What a group's decks must meet or clear, each where it lies, in the ring's cells.
#[derive(Default)]
struct DeckReqs {
    /// The ground under each cell, and its member.
    grounds: Vec<([f64; 2], i32, usize)>,
    /// What a cell must clear, that capped so the deck can still come down to every anchor, and
    /// whether another deck sets it (a deck under it to clear).
    needs: Vec<([f64; 2], i32, f64, bool)>,
    /// An end the deck comes down to: the ground, or a deck it joins (`true`); and its member.
    anchors: Vec<([f64; 2], i32, bool, usize)>,
    /// A free end, which stays up for the ramp that carries the way on.
    frees: Vec<[f64; 2]>,
    /// Every end that is the structure's own.
    bounds: Vec<[f64; 2]>,
}

/// A member's way whole: where it starts and its heading, along which its deck is solved, whether
/// it keeps to that heading, its layer and its length in cells.
struct DeckMember {
    origin: [f64; 2],
    dir: [f64; 2],
    follows: bool,
    level: i64,
    length: f64,
}

/// A block's deck: the least concave line over what it must meet, along its member's heading, or
/// one level where a way about it winds off its own.
enum DeckBlock {
    Hull(Vec<(f64, f64)>),
    Level(i32),
}

impl DeckBlock {
    fn at(&self, u: f64) -> f64 {
        match self {
            DeckBlock::Hull(hull) => hull_at(hull, u),
            DeckBlock::Level(y) => f64::from(*y),
        }
    }
}

/// Run `r`'s deck cell by cell, member `m` of its group (`DECK_BLOCK`).
fn deck_of(r: &Run, m: usize, members: &[DeckMember], reqs: &DeckReqs, units: Units) -> Vec<i32> {
    let me = &members[m];
    let mut blocks: FnvHashMap<usize, DeckBlock> = FnvHashMap::default();
    (0..r.cell.len())
        .map(|i| {
            // its place along the way whole, which no ring moves
            let a = r.k0 + i;
            let (k, t) = (a / DECK_BLOCK, (a % DECK_BLOCK) as f64 / DECK_BLOCK as f64);
            let u = along(me, absolute(r.at[i]));
            let mut at = |k: usize| blocks.entry(k).or_insert_with(|| deck_block(r, k, m, members, reqs, units)).at(u);
            let y = if t > 0.0 { at(k) * (1.0 - t) + at(k + 1) * t } else { at(k) };
            round(y) as i32
        })
        .collect()
}

/// A point in DB coordinates as cells from the world's corner: the same bits on every tile, where a
/// ring's own cells (`Run::cell`), taken from its corner, differ in their last bits from one tile's to
/// the next's, enough to move what lies on a window's edge.
fn absolute(p: [f64; 2]) -> [f64; 2] {
    [u_from_mer(p[0]) / CELL_U, u_from_mer(p[1]) / CELL_U]
}

/// Where `p` lies along a member's heading.
fn along(me: &DeckMember, p: [f64; 2]) -> f64 {
    (p[0] - me.origin[0]) * me.dir[0] + (p[1] - me.origin[1]) * me.dir[1]
}

/// Block `k` of run `r`'s deck, from what its group must meet about its middle: as Arnis's structure,
/// but over the windows (`DECK_WINDOW`, `DECK_NEAR`), not the whole. The ground floors the deck and is
/// no point of its line, so a deck with one thing to clear in sight holds its height over it rather
/// than dipping to the ground at the window's edge; where an end is in sight, it comes down to it.
fn deck_block(r: &Run, k: usize, m: usize, members: &[DeckMember], reqs: &DeckReqs, units: Units) -> DeckBlock {
    let me = &members[m];
    let c = r.whole.at(((k * DECK_BLOCK) as f64).min(r.whole.length()));
    let within = |p: &[f64; 2], d: usize| libm::hypot(p[0] - c[0], p[1] - c[1]) <= d as f64;
    // what the source gives within the window, what another deck sets within the near
    let reach = |set_by_deck: bool| if set_by_deck { DECK_NEAR } else { DECK_WINDOW };
    let grounds: Vec<&([f64; 2], i32, usize)> = reqs.grounds.iter().filter(|g| within(&g.0, DECK_WINDOW)).collect();
    let anchors: Vec<&([f64; 2], i32, bool, usize)> = reqs.anchors.iter().filter(|a| within(&a.0, reach(a.2))).collect();
    let needs: Vec<&([f64; 2], i32, f64, bool)> = reqs.needs.iter().filter(|n| within(&n.0, reach(n.3))).collect();
    let (Some(tmax), Some(tmin)) = (grounds.iter().map(|g| g.1).max(), grounds.iter().map(|g| g.1).min()) else {
        return DeckBlock::Level(0);
    };
    let mut about = vec![false; members.len()];
    for g in &grounds {
        about[g.2] = true;
    }
    let about = || members.iter().zip(&about).filter(|(_, &a)| a).map(|(m, _)| m);
    // a long flat span rises to its layer
    let total: f64 = about().map(|m| m.length).sum();
    let flat = reqs.bounds.iter().any(|b| within(b, DECK_WINDOW)) && tmax - tmin < units.c(FLAT_TERRAIN_DIP_THRESHOLD_M) && total >= f64::from(units.c(SHORT_BRIDGE_LENGTH_M));
    let clearance = if flat {
        about().map(|m| m.level).max().unwrap_or(0) as i32 * units.c(LAYER_HEIGHT_STEP_M)
    } else {
        0
    };
    let plateau = (clearance > 0).then_some(tmax + clearance);
    if !about().all(|m| m.follows) {
        return DeckBlock::Level(needs.iter().map(|n| n.1).chain(plateau).fold(tmax, i32::max));
    }
    let raised = anchors
        .iter()
        .filter(|a| a.2)
        .map(|a| a.1)
        .chain(needs.iter().map(|n| round(n.2) as i32))
        .chain(plateau)
        .fold(tmax, i32::max);
    let u = |p: [f64; 2]| along(me, p);
    // a joint holds every deck about it; an end at the ground only its own way's, which ramps down to it,
    // where Arnis's one line over the whole passes over an end in the middle of a structure
    let mut pts: Vec<(f64, f64)> = anchors.iter().filter(|a| a.2 || a.3 == m).map(|a| (u(a.0), f64::from(a.1))).collect();
    pts.extend(needs.iter().map(|n| (u(n.0), n.2)));
    if let Some(top) = plateau {
        let (lo, hi) = grounds.iter().map(|g| u(g.0)).fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
        let span = (hi - lo).max(0.0);
        let ramp = ramp_len(span, units).min(span / 2.0);
        let mut placed = false;
        for g in &grounds {
            if anchors.iter().all(|a| libm::hypot(g.0[0] - a.0[0], g.0[1] - a.0[1]) >= ramp) {
                pts.push((u(g.0), f64::from(top)));
                placed = true;
            }
        }
        if !placed {
            pts.push(((lo + hi) / 2.0, f64::from(top)));
        }
    }
    pts.extend(reqs.frees.iter().filter(|f| within(f, DECK_WINDOW)).map(|f| (u(*f), f64::from(raised))));
    let Some(top) = pts.iter().map(|p| p.1).reduce(f64::max) else {
        // nothing to meet or clear in sight: the deck stands on its highest ground about it
        return DeckBlock::Level(tmax);
    };
    // where no end of its own way is in sight, the structure goes on past the window, and the deck
    // holds the highest it meets in sight rather than falling to something low at the window's edge
    // that a whole structure's line would pass over; an end in sight it comes down to
    let uc = u(c);
    let ends: Vec<f64> = anchors.iter().filter(|a| !a.2 && a.3 == m).map(|a| u(a.0)).collect();
    if !ends.iter().any(|&e| e < uc) {
        pts.push((uc - DECK_WINDOW as f64, top));
    }
    if !ends.iter().any(|&e| e > uc) {
        pts.push((uc + DECK_WINDOW as f64, top));
    }
    DeckBlock::Hull(upper_hull(pts))
}

fn bridges(src: &Source, runs: &[Run], obstacles: &Obstacles, units: Units, lines: &mut Vec<(Line, Drawn)>) -> Bridges {
    let road: Vec<usize> = (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Bridge && !runs[q].rail()).collect();
    let mut uf = Uf((0..road.len()).collect());
    // shared ends at one layer, then decks side by side
    let mut by_node: BTreeMap<&End, Vec<usize>> = BTreeMap::new();
    for (i, &q) in road.iter().enumerate() {
        for e in &runs[q].ends {
            by_node.entry(e).or_default().push(i);
        }
    }
    for members in by_node.values() {
        for (a, &i) in members.iter().enumerate() {
            for &j in &members[a + 1..] {
                if effective_layer(&runs[road[i]]) == effective_layer(&runs[road[j]]) {
                    uf.union(i, j);
                }
            }
        }
    }
    // two decks whose cells lie further apart than any deck beside another (`side_by_side`'s
    // farthest) are not weighed
    let boxes: Vec<[[f64; 2]; 2]> = road.iter().map(|&q| cells_box(&runs[q].whole.points)).collect();
    let reach = |q: usize| f64::from(ways::reach((runs[q].width_u / CELL_U) as f32));
    let farthest = |a: usize, b: usize| (reach(a) + reach(b) + f64::from(units.c(SIDE_DECK_EDGE_GAP_M))).max(f64::from(units.c(DUAL_CARRIAGEWAY_MAX_DISTANCE_M)));
    for i in 0..road.len() {
        for j in i + 1..road.len() {
            let apart = (0..2).map(|k| (boxes[j][0][k] - boxes[i][1][k]).max(boxes[i][0][k] - boxes[j][1][k])).fold(0.0, f64::max);
            if apart <= farthest(road[i], road[j]) && uf.find(i) != uf.find(j) && side_by_side(&runs[road[i]], &runs[road[j]], units) {
                uf.union(i, j);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &r) in road.iter().enumerate() {
        groups.entry(uf.find(i)).or_default().push(r);
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().collect();
    groups.sort_by_key(|g| (g.iter().map(|&q| effective_layer(&runs[q])).max().unwrap_or(0), runs[g[0]].piece.id()));

    let mut bridge_nodes: FnvHashMap<&End, Vec<usize>> = FnvHashMap::default();
    for q in (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Bridge) {
        for e in &runs[q].ends {
            bridge_nodes.entry(e).or_default().push(q);
        }
    }
    let mut separation: Separation = FnvHashMap::default();
    let mut joints: FnvHashMap<End, i32> = FnvHashMap::default();
    let mut claimed: FnvHashSet<PartId> = FnvHashSet::default();
    let mut out = Vec::new();
    for g in &groups {
        let mut node_members: FnvHashMap<&End, usize> = FnvHashMap::default();
        let mut group_nodes: FnvHashSet<&End> = FnvHashSet::default();
        for &q in g {
            let seen: FnvHashSet<&End> = runs[q].nodes.iter().collect();
            for n in &seen {
                *node_members.entry(n).or_default() += 1;
            }
            group_nodes.extend(seen);
        }
        // an end the ring cut is none (the module's rules): no anchor, no free end, no ramp
        let is_boundary = |n: &End| !matches!(n, End::Ring(..)) && node_members.get(n).copied().unwrap_or(0) <= 1;
        let joins_other = |n: &End| bridge_nodes.get(n).is_some_and(|qs| qs.iter().any(|q| !g.contains(q)));

        // ramps: a way at a free end, raised and no bridge, carries the deck down
        let mut ramp_ends: FnvHashSet<&End> = FnvHashSet::default();
        let mut pending: Vec<(PartId, Vec<[f64; 2]>, End, i32)> = Vec::new();
        for &q in g {
            for e in &runs[q].ends {
                if !is_boundary(e) {
                    continue;
                }
                for part in src.way_ends.get(e).into_iter().flatten() {
                    let way = &src.ways[part];
                    // a road's alone, never indoors (Arnis `is_ramp_candidate`)
                    let t = way.row.tags;
                    if way.row.subtype != "road" || t.bridge || t.tunnel || t.indoor || !t.raised || claimed.contains(part) {
                        continue;
                    }
                    let Some((line, far_end)) = way_from(way, e) else { continue };
                    let cells = cells_along(&line);
                    // a ramp that leaves the ring is none: its far end is not known
                    if group_nodes.contains(far_end) || !cells.iter().all(|&p| src.inside(p)) {
                        continue;
                    }
                    let Ok(ground) = sample_cells(&src.ground, &[line[line.len() - 1]]) else { continue };
                    pending.push((part.clone(), cells, e.clone(), ground[0]));
                    claimed.insert(part.clone());
                    ramp_ends.insert(e);
                }
            }
        }
        let anchors_end = |n: &End| is_boundary(n) && !ramp_ends.contains(n) && !joins_other(n);

        // what the deck must meet or clear, each where it lies, in the ring's cells
        let mut reqs = DeckReqs::default();
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            let last = r.cell.len() - 1;
            for (e, i) in [(&r.ends[0], 0), (&r.ends[1], last)] {
                if is_boundary(e) {
                    reqs.bounds.push(absolute(r.at[i]));
                }
                if anchors_end(e) {
                    reqs.anchors.push((absolute(r.at[i]), r.terrain[i], false, m));
                } else if let Some(&y) = joints.get(e) {
                    reqs.anchors.push((absolute(r.at[i]), y, true, m));
                } else if is_boundary(e) {
                    reqs.frees.push(absolute(r.at[i]));
                }
            }
        }
        // a clearance is capped so the deck can still come down to every anchor
        // an end at the ground caps from wherever it lies; a joint, another deck's, from near (`DECK_NEAR`)
        let reachable = |p: [f64; 2]| {
            reqs.anchors
                .iter()
                .map(|&(a, ya, joint, _)| (libm::hypot(p[0] - a[0], p[1] - a[1]), ya, joint))
                .filter(|&(d, _, joint)| !joint || d <= DECK_NEAR as f64)
                .map(|(d, ya, _)| f64::from(ya) + d * MAX_CLEARANCE_SLOPE)
                .fold(f64::INFINITY, f64::min)
        };
        let mut needs = Vec::new();
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            let layer = effective_layer(r);
            for (i, &c) in r.cell.iter().enumerate() {
                let (at, ground) = (absolute(r.at[i]), r.terrain[i]);
                reqs.grounds.push((at, ground, m));
                let mut need = ground;
                let (p, n) = (r.cell[i.saturating_sub(1)], r.cell[(i + 1).min(r.cell.len() - 1)]);
                for o in obstacles.at(c) {
                    // a way meeting the structure runs at its level, not under it
                    if ways::covers(o.width, dist_to_segment(c, o.a, o.b) as f32) && crosses([n[0] - p[0], n[1] - p[1]], o) && !o.nodes.iter().any(|n| group_nodes.contains(n)) {
                        need = need.max(ground + o.headroom);
                    }
                }
                if need > ground {
                    needs.push((at, need, ground, false));
                }
                if let Some(&(lower, lower_layer)) = separation.get(&cell_key(c)) {
                    let over = lower + units.c(f64::from(STACKED_DECK_HEADROOM_M));
                    if lower_layer < layer && over > ground {
                        needs.push((at, over, ground, true));
                    }
                }
            }
        }
        reqs.needs = needs
            .into_iter()
            .map(|(c, need, ground, stacked)| (c, need, f64::from(need).min(reachable(c)).max(f64::from(ground)), stacked))
            .collect();
        let members: Vec<DeckMember> = g
            .iter()
            .map(|&q| {
                let w = &runs[q].whole;
                let (s, e) = (w.points[0], w.points[w.points.len() - 1]);
                let d = libm::hypot(e[0] - s[0], e[1] - s[1]);
                let dir = if d > 0.0 { [(e[0] - s[0]) / d, (e[1] - s[1]) / d] } else { [1.0, 0.0] };
                let along = |c: &[f64; 2]| (c[0] - s[0]) * dir[0] + (c[1] - s[1]) * dir[1];
                DeckMember {
                    origin: s,
                    dir,
                    // whether the way runs along its own heading, judged on the way whole
                    follows: d >= 1.0 && is_monotonic(&w.points.iter().map(along).collect::<Vec<f64>>(), f64::from(units.c(PROFILE_MONOTONIC_TOLERANCE_M))),
                    level: runs[q].piece.way.tags.level.map_or(0, |l| l.max(0)),
                    length: w.length(),
                }
            })
            .collect();
        let mut ys: Vec<Vec<i32>> = g.iter().enumerate().map(|(m, &q)| deck_of(&runs[q], m, &members, &reqs, units)).collect();

        // an end above the ground ramps down to it inside the member
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            let total = ys[m].len();
            let (gs, ge) = (r.terrain[0], r.terrain[total - 1]);
            let ramp_s = anchors_end(&r.ends[0]) && ys[m][0] > gs + 1;
            let ramp_e = anchors_end(&r.ends[1]) && ys[m][total - 1] > ge + 1;
            let cap = if ramp_s && ramp_e { (total / 2).max(1) } else { total - 1 };
            // as long as the way whole asks, not as long as the stretch the ring left
            let len = (ramp_len(members[m].length, units) as usize).clamp(1, cap);
            if ramp_s {
                let top = ys[m][len];
                for (k, y) in ys[m][..len].iter_mut().enumerate() {
                    *y = (*y).min(round(f64::from(gs) + f64::from(top - gs) * k as f64 / len as f64) as i32);
                }
            }
            if ramp_e {
                let top = ys[m][total - 1 - len];
                for k in 0..len {
                    let y = &mut ys[m][total - 1 - k];
                    *y = (*y).min(round(f64::from(ge) + f64::from(top - ge) * k as f64 / len as f64) as i32);
                }
            }
            for (y, &t) in ys[m].iter_mut().zip(&r.terrain) {
                *y = (*y).max(t);
            }
        }

        // a ramp runs from the deck's end down to the ground at its far end
        for (part, cells, e, ground) in pending {
            let deck_y = g
                .iter()
                .enumerate()
                .find_map(|(m, &q)| {
                    if runs[q].ends[0] == e {
                        ys[m].first().copied()
                    } else if runs[q].ends[1] == e {
                        ys[m].last().copied()
                    } else {
                        None
                    }
                })
                .unwrap_or(ground);
            let way = &src.ways[&part];
            let denom = (cells.len() - 1).max(1) as f64;
            lines.push(graded(
                way,
                cells
                    .iter()
                    .enumerate()
                    .map(|(k, p)| [p[0], p[1], round(f64::from(deck_y) + f64::from(ground - deck_y) * k as f64 / denom)])
                    .collect(),
                [true, true],
                way.row.span,
            ));
        }

        // what the structures after it meet and clear
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            for (e, y) in [(&r.ends[0], ys[m][0]), (&r.ends[1], ys[m][ys[m].len() - 1])] {
                let j = joints.entry(e.clone()).or_insert(y);
                *j = (*j).max(y);
            }
            let layer = effective_layer(r);
            let width = (r.width_u / CELL_U) as f32;
            let around = within(ways::reach(width));
            for (&c, &y) in r.cell.iter().zip(&ys[m]) {
                let (x, z) = cell_key(c);
                for &[dx, dz] in &around {
                    let e = separation.entry((x + dx, z + dz)).or_insert((y, layer));
                    if y > e.0 {
                        *e = (y, layer);
                    }
                }
            }
            out.push((q, std::mem::take(&mut ys[m])));
        }
    }
    Bridges {
        ys: out,
        separation,
        ramp_ways: claimed,
    }
}

// ---- rail bridges: on a road deck, or one level a viaduct with ramps at its free ends ----

fn rail_bridges(runs: &[Run], obstacles: &Obstacles, separation: &Separation, units: Units) -> Vec<(usize, Vec<i32>)> {
    let rail: Vec<usize> = (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Bridge && runs[q].rail()).collect();
    let mut out = Vec::new();
    let mut ends_count: FnvHashMap<&End, usize> = FnvHashMap::default();
    for &q in &rail {
        for e in &runs[q].ends {
            *ends_count.entry(e).or_default() += 1;
        }
    }
    // what each track cell rides: the nearest road deck of its layer within reach
    let reach = i64::from(units.c(RAIL_ON_DECK_REACH_M));
    let on: Vec<Vec<Option<i32>>> = rail
        .iter()
        .map(|&q| {
            let layer = effective_layer(&runs[q]);
            runs[q]
                .cell
                .iter()
                .map(|&c| {
                    let (x, z) = cell_key(c);
                    let mut best: Option<(i64, i32)> = None;
                    for dx in -reach..=reach {
                        for dz in -reach..=reach {
                            if let Some(&(y, l)) = separation.get(&(x + dx, z + dz)) {
                                let d = dx.abs() + dz.abs();
                                if l == layer && best.is_none_or(|(bd, _)| d < bd) {
                                    best = Some((d, y));
                                }
                            }
                        }
                    }
                    best.map(|b| b.1)
                })
                .collect()
        })
        .collect();
    // joined through every rail bridge, a carried one included, as Arnis's `rail_uf`
    let mut uf = Uf((0..rail.len()).collect());
    let mut at_end: FnvHashMap<(&End, i64), usize> = FnvHashMap::default();
    for (i, &q) in rail.iter().enumerate() {
        for e in &runs[q].ends {
            match at_end.get(&(e, effective_layer(&runs[q]))) {
                Some(&j) => uf.union(i, j),
                None => {
                    at_end.insert((e, effective_layer(&runs[q])), i);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..rail.len() {
        groups.entry(uf.find(i)).or_default().push(i);
    }
    for g in groups.into_values() {
        // each cell of the group: where it lies, its ground, what it must clear of the source's ways and
        // of a deck under it, and its layer
        let mut cells: Vec<([f64; 2], i32, i32, i32, i64)> = Vec::new();
        for &i in &g {
            let r = &runs[rail[i]];
            let layer = effective_layer(r);
            for (j, &c) in r.cell.iter().enumerate() {
                let ground = r.terrain[j];
                let (mut floor, mut over) = (i32::MIN, i32::MIN);
                let (p, n) = (r.cell[j.saturating_sub(1)], r.cell[(j + 1).min(r.cell.len() - 1)]);
                for o in obstacles.at(c) {
                    if ways::covers(o.width, dist_to_segment(c, o.a, o.b) as f32) && crosses([n[0] - p[0], n[1] - p[1]], o) && !o.nodes.iter().any(|n| r.nodes.contains(n)) {
                        floor = floor.max(ground + o.headroom);
                    }
                }
                if let Some(&(lower, lower_layer)) = separation.get(&cell_key(c)) {
                    if lower_layer < layer {
                        over = lower + units.c(f64::from(STACKED_DECK_HEADROOM_M));
                    }
                }
                cells.push((absolute(r.at[j]), ground, floor, over, layer));
            }
        }
        let within = |c: [f64; 2], p: [f64; 2], d: usize| libm::hypot(p[0] - c[0], p[1] - c[1]) <= d as f64;
        let near = |c: [f64; 2], p: [f64; 2]| within(c, p, DECK_WINDOW);
        // a flat span lifts clear, a level more a layer; a valley span sits on its banks: over the
        // group's cells about the block's middle, as a road deck's (`DECK_BLOCK`), a deck under it
        // cleared only near
        let level_at = |c: [f64; 2]| {
            let (mut tmax, mut tmin, mut floor, mut layer) = (i32::MIN, i32::MAX, i32::MIN, 0);
            for &(p, ground, f, over, l) in cells.iter().filter(|x| near(c, x.0)) {
                let over = if within(c, p, DECK_NEAR) { over } else { i32::MIN };
                (tmax, tmin, floor, layer) = (tmax.max(ground), tmin.min(ground), floor.max(f).max(over), layer.max(l));
            }
            if tmax == i32::MIN {
                return None;
            }
            Some(
                if tmax - tmin < units.c(RAIL_BRIDGE_DIP_THRESHOLD_M) {
                    tmax + units.c(RAIL_BRIDGE_FLAT_CLEARANCE_M) + (layer as i32 - 1).max(0) * units.c(LAYER_HEIGHT_STEP_M)
                } else {
                    tmax
                }
                .max(floor),
            )
        };
        for &i in &g {
            let r = &runs[rail[i]];
            let total = r.cell.len();
            let last = total - 1;
            // the deck it rides, a gap in it taken from the cell before or after
            let mut rode = on[i].clone();
            for j in 1..total {
                if rode[j].is_none() {
                    rode[j] = rode[j - 1];
                }
            }
            for j in (0..last).rev() {
                if rode[j].is_none() {
                    rode[j] = rode[j + 1];
                }
            }
            // a track along a road deck rides it where nearly all of it does within a window of the
            // block's middle, and is one level a viaduct elsewhere
            let block = |k: usize| {
                let c = r.whole.at(((k * DECK_BLOCK) as f64).min(r.whole.length()));
                let mine: Vec<bool> = (0..total).filter(|&j| near(c, absolute(r.at[j]))).map(|j| on[i][j].is_some()).collect();
                let rides = !mine.is_empty() && mine.iter().filter(|&&s| s).count() * 10 >= mine.len() * 9;
                (rides, if rides { None } else { level_at(c) })
            };
            let mut blocks: FnvHashMap<usize, (bool, Option<i32>)> = FnvHashMap::default();
            let mut riding = vec![false; total];
            let deck: Vec<i32> = (0..total)
                .map(|j| {
                    let a = r.k0 + j;
                    let (k, t) = (a / DECK_BLOCK, (a % DECK_BLOCK) as f64 / DECK_BLOCK as f64);
                    let ride = rode[j].unwrap_or(r.terrain[j]);
                    let mut at = |k: usize| {
                        let (rides, level) = *blocks.entry(k).or_insert_with(|| block(k));
                        (rides, if rides { f64::from(ride) } else { f64::from(level.unwrap_or(ride)) })
                    };
                    let (rides, y) = at(k);
                    riding[j] = rides;
                    round(if t > 0.0 { y * (1.0 - t) + at(k + 1).1 * t } else { y }) as i32
                })
                .collect();
            // each way ramps from its free ends where it is a viaduct, no steeper than a cell a cell
            // (Arnis `generate_rail_bridge`); an end the ring cut is none, and the viaduct carries on there
            let internal = [ends_count[&r.ends[0]] > 1 || r.clipped[0] || riding[0], ends_count[&r.ends[1]] > 1 || r.clipped[1] || riding[last]];
            let (gs, ge) = (r.terrain[0], r.terrain[last]);
            let mut needed = 0;
            if !internal[0] {
                needed = needed.max((deck[0] - gs).max(0) as usize);
            }
            if !internal[1] {
                needed = needed.max((deck[last] - ge).max(0) as usize);
            }
            // as long as the way whole asks, not as long as the stretch the ring left
            let ramp = ((r.whole.length() * RAIL_BRIDGE_RAMP_FRACTION) as usize)
                .clamp(units.c(RAIL_BRIDGE_RAMP_M[0]) as usize, units.c(RAIL_BRIDGE_RAMP_M[1]) as usize)
                .max(needed + 1);
            let denom = ramp.saturating_sub(1).max(1) as f64;
            let ys = (0..total)
                .map(|k| {
                    let s = if internal[0] {
                        deck[k]
                    } else {
                        round(f64::from(gs) + f64::from(deck[k] - gs) * (k as f64 / denom).min(1.0)) as i32
                    };
                    let e = if internal[1] {
                        deck[k]
                    } else {
                        round(f64::from(ge) + f64::from(deck[k] - ge) * ((last - k) as f64 / denom).min(1.0)) as i32
                    };
                    s.min(e).max(r.terrain[k])
                })
                .collect();
            out.push((rail[i], ys));
        }
    }
    out
}

// ---- road tunnels: approaches sink into the portals, the bore as deep as its cover asks ----

/// `drop` cells under grade at cell `anchor`, easing back over `run` cells a cell.
#[derive(Clone, Copy)]
struct Claim {
    anchor: i32,
    drop: i32,
    run: i32,
}

/// How far under its cover a road tunnel at `level` is bored, and so how far its portals' approaches
/// sink, in cells: Arnis's cover drop, and a layer's drop for each layer below the first.
fn tunnel_drop(level: i64, units: Units) -> i32 {
    units.c(TUNNEL_COVER_DROP_M) + (-(level + 1)).clamp(0, TUNNEL_MAX_LAYERS_BELOW) as i32 * units.c(TUNNEL_LAYER_DROP_M)
}

fn road_tunnels(src: &Source, runs: &[Run], bridges: &Bridges, units: Units, lines: &mut Vec<(Line, Drawn)>) -> Vec<(usize, Vec<i32>, Vec<bool>)> {
    let ramp_ways = &bridges.ramp_ways;
    let tunnels: Vec<usize> = (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Tunnel && !runs[q].rail()).collect();
    // a node two tunnels share, or an end the ring cut, stays at depth
    let mut owner: FnvHashMap<&End, PartId> = FnvHashMap::default();
    let mut shared: FnvHashSet<&End> = FnvHashSet::default();
    for &q in &tunnels {
        for n in &runs[q].nodes {
            let id = runs[q].piece.id();
            match owner.get(n) {
                Some(o) if *o != id => {
                    shared.insert(n);
                }
                None => {
                    owner.insert(n, id);
                }
                _ => {}
            }
        }
    }
    let internal = |q: usize, e: usize| runs[q].clipped[e] || shared.contains(&runs[q].ends[e]);
    let drop_of = |r: &Run| tunnel_drop(r.piece.way.tags.level.unwrap_or(0), units);

    // ---- a portal on a road deck, at its end or along it, opens at the deck's height there, where Arnis
    // leaves it at the terrain under the deck: the bore ramps down from it, with no approach to sink ----
    let mut deck: FnvHashMap<&End, i32> = FnvHashMap::default();
    for &q in &tunnels {
        for e in (0..2).filter(|&e| !internal(q, e)) {
            let (node, p) = (&runs[q].ends[e], runs[q].at[if e == 0 { 0 } else { runs[q].at.len() - 1 }]);
            let on = bridges
                .ys
                .iter()
                .filter(|(b, _)| runs[*b].nodes.contains(node))
                .filter_map(|(b, ys)| {
                    let near = |k: &usize| libm::hypot(runs[*b].at[*k][0] - p[0], runs[*b].at[*k][1] - p[1]);
                    // the run of the deck that passes it: a cell of it within a cell of the portal
                    (0..ys.len()).min_by(|i, j| near(i).total_cmp(&near(j))).filter(|k| near(k) <= mer_from_u(CELL_U)).map(|k| ys[k])
                })
                .max();
            if let Some(y) = on {
                deck.insert(node, y);
            }
        }
    }

    // ---- the approaches: ways at grade from a portal, as far as the drop needs and the road goes on ----
    let portal_nodes: FnvHashSet<&End> = tunnels.iter().flat_map(|&q| (0..2).filter(move |&e| !internal(q, e)).map(move |e| &runs[q].ends[e])).collect();
    let surface = |part: &PartId| {
        src.ways
            .get(part)
            .filter(|w| !w.row.tags.bridge && !w.row.tags.tunnel && w.row.subtype == "road" && !ramp_ways.contains(part))
    };
    let step = mer_from_u(CELL_U);
    // a way's last cell as `cells_along` counts them
    let last_cell = |part: &PartId| libm::ceil(src.ways[part].line.length() / step).max(1.0) as i32;
    // a way's cells from the end it is entered at, as far as its ground is held and no further than
    // `cap`, which is all a descent can use, and whether it stops short of its far end
    let in_ring = |part: &PartId, from_start: bool, cap: i32| {
        let (line, last) = (&src.ways[part].line, last_cell(part));
        let mut n = 0;
        while n <= last && n < cap {
            let k = if from_start { n } else { last - n };
            if !src.held(line.at((f64::from(k) * step).min(line.length()))) {
                break;
            }
            n += 1;
        }
        (n, n <= last)
    };
    let mut claims: BTreeMap<PartId, Vec<Claim>> = BTreeMap::new();
    let mut drops: FnvHashMap<End, i32> = FnvHashMap::default();
    for &q in &tunnels {
        let r = &runs[q];
        let want = drop_of(r);
        let walking = is_pedestrian(r.piece.way.class.as_deref().unwrap_or(""));
        for e in 0..2 {
            let node = &r.ends[e];
            if internal(q, e) || deck.contains_key(node) || drops.contains_key(node) || want <= 0 {
                continue;
            }
            let budget = want * TUNNEL_RAMP_RUN;
            let mut taken: FnvHashSet<PartId> = FnvHashSet::default();
            let mut branches: Vec<Vec<(PartId, bool, i32, i32)>> = Vec::new();
            let eligible = |at: &End, taken: &FnvHashSet<PartId>, claims: &BTreeMap<PartId, Vec<Claim>>| -> Vec<PartId> {
                let mut out: Vec<PartId> = src
                    .way_ends
                    .get(at)
                    .into_iter()
                    .flatten()
                    .filter(|g| !taken.contains(*g) && claims.get(*g).is_none_or(|c| c.len() < 2))
                    .filter(|g| surface(g).is_some_and(|w| is_pedestrian(w.row.class.as_deref().unwrap_or("")) == walking))
                    .cloned()
                    .collect();
                out.sort();
                out.dedup();
                out
            };
            for first in eligible(node, &taken, &claims) {
                if taken.contains(&first) {
                    continue;
                }
                let mut chain = Vec::new();
                let (mut part, mut at, mut dist) = (first, node.clone(), 0);
                loop {
                    let w = &src.ways[&part];
                    let at_start = w.row.ends[0] == at;
                    // past the budget no cell changes the descent: `avail` is as much as it needs
                    let (len, leaves) = in_ring(&part, at_start, budget - dist + 1);
                    chain.push((part.clone(), at_start, dist, len));
                    taken.insert(part.clone());
                    dist += len - 1;
                    at = w.row.ends[usize::from(at_start)].clone();
                    if leaves || dist >= budget || chain.len() >= TUNNEL_APPROACH_WAYS || portal_nodes.contains(&at) {
                        break;
                    }
                    // only where the road goes on: a junction stays at grade
                    let next = eligible(&at, &taken, &claims);
                    if next.len() != 1 {
                        break;
                    }
                    part = next[0].clone();
                }
                branches.push(chain);
            }
            // every branch takes the same descent, or one ends in a cliff
            let Some(avail) = branches.iter().filter_map(|b| b.last().map(|(_, _, d0, len)| d0 + len - 1)).min() else {
                continue;
            };
            let drop = want.min(avail);
            if drop <= 0 {
                continue;
            }
            let run = (avail / drop).clamp(1, TUNNEL_RAMP_RUN);
            for (part, at_start, d0, _) in branches.into_iter().flatten() {
                let last = last_cell(&part);
                let c = claims.entry(part).or_default();
                if c.len() < 2 {
                    c.push(Claim {
                        anchor: if at_start { -d0 } else { last + d0 },
                        drop,
                        run,
                    });
                }
            }
            drops.insert(node.clone(), drop);
        }
    }
    for (part, cs) in &claims {
        let (w, last) = (&src.ways[part], last_cell(part));
        let along = |k: i32| (f64::from(k) * step).min(w.line.length());
        // where it descends, within a claim's drop of its anchor, to the cell either side where it is
        // back at grade; past that it is a way at grade, the ground's to paint
        let mut spans: Vec<[i32; 2]> = cs
            .iter()
            .map(|c| [(c.anchor - c.drop * c.run).max(0), (c.anchor + c.drop * c.run).min(last)])
            .filter(|[a, b]| a < b)
            .collect();
        spans.sort_unstable();
        let mut merged: Vec<[i32; 2]> = Vec::new();
        for [a, b] in spans {
            match merged.last_mut() {
                Some(m) if a <= m[1] => m[1] = m[1].max(b),
                _ => merged.push([a, b]),
            }
        }
        for [a, b] in merged {
            // each stretch of it whose ground is held, the cells past it not read
            let mut k = a;
            while k <= b {
                let from = k;
                let mut cells = Vec::new();
                while k <= b && src.held(w.line.at(along(k))) {
                    cells.push(w.line.at(along(k)));
                    k += 1;
                }
                if cells.len() >= 2 {
                    if let Ok(terrain) = sample_cells(&src.ground, &cells) {
                        let points = (from..k)
                            .zip(cells.iter().zip(&terrain))
                            .map(|(k, (p, &t))| {
                                let drop = cs.iter().map(|c| c.drop - (k - c.anchor).abs() / c.run).max().unwrap_or(0).max(0);
                                [p[0], p[1], f64::from(t - drop)]
                            })
                            .collect();
                        let fraction = |k: i32| w.row.span[0] + along(k) / w.row.segment_length;
                        lines.push(graded(w, points, [from == 0, k - 1 == last], [fraction(from), fraction(k - 1)]));
                    }
                }
                k += 1;
            }
        }
    }

    // ---- each bore (Arnis `generate_highway_tunnel_shell`); the world has no bedrock to stop it ----
    let roof = units.c(TUNNEL_ROOF_COVER_M);
    let mut out = Vec::new();
    for &q in &tunnels {
        let r = &runs[q];
        let n = r.cell.len();
        let last = n - 1;
        // the lowest ground over the bore's footprint decides its roof
        let wall = within(ways::footprint((r.width_u / CELL_U) as f32));
        let cover: Vec<i32> =
            r.at.iter()
                .zip(&r.terrain)
                .map(|(p, &t)| {
                    let around: Vec<[f64; 2]> = wall
                        .iter()
                        .map(|&[dx, dz]| [p[0] + mer_from_u(dx as f64 * CELL_U), p[1] + mer_from_u(dz as f64 * CELL_U)])
                        .filter(|q| src.loaded.contains(&tile_of(*q)))
                        .collect();
                    sample_cells(&src.ground, &around).map_or(t, |v| v.into_iter().fold(t, i32::min))
                })
                .collect();
        let (internal_s, internal_e) = (internal(q, 0), internal(q, 1));
        let deck_s = if internal_s { None } else { deck.get(&r.ends[0]).copied() };
        let deck_e = if internal_e { None } else { deck.get(&r.ends[1]).copied() };
        let (gs, ge) = (deck_s.unwrap_or(r.terrain[0]), deck_e.unwrap_or(r.terrain[last]));
        let drop_cover = drop_of(r);
        let start_drop = if internal_s { 0 } else { drops.get(&r.ends[0]).copied().unwrap_or(0) };
        let end_drop = if internal_e { 0 } else { drops.get(&r.ends[1]).copied().unwrap_or(0) };
        // A bore no longer than a tile is Arnis's: under the straight run between its portals, at most
        // as high as its cover leaves room for, in one valley. Every tile that draws any of it holds
        // all of it in its ring, so all of them solve it alike.
        //
        // A longer one is not: a tile that draws it may see neither portal, and the straight run
        // between them is not a ring's to know. Its ring cuts it, and where the ring cuts is no end of
        // it, nor a height to draw a line from: two neighbours cut it in different places, and a
        // floor that leaned on the cut would differ where they meet, though both see the cells there.
        // So the floor of a long bore leans on nothing further than a tile (`TILE_CELLS`) from each
        // cell, which every ring holds about what its tile draws: the highest floor under the cover by
        // its drop that rises from the lowest such floor within a tile at `TUNNEL_FAR_GRADE` at most
        // (`envelope`), raised to whichever portal it sees. Reading past the ring to find the portals
        // instead costs every tile reads it is not owed, and is not done.
        let long = libm::ceil(r.piece.way.length() / step) > f64::from(TILE_CELLS);
        let denom = last.max(1) as f64;
        let grade = |i: usize| round(f64::from(gs) + f64::from(ge - gs) * i as f64 / denom) as i32;
        let floor = if long {
            let room: Vec<i32> = cover.iter().map(|c| c - drop_cover).collect();
            envelope(&room, TUNNEL_FAR_GRADE, TILE_CELLS as usize).into_iter().map(|y| round(y) as i32).collect()
        } else {
            (0..n).map(|i| grade(i).min(cover[i] - drop_cover)).collect::<Vec<i32>>()
        };
        // the ground caps the bore, but where a deck's portal holds it over the ground, ramping down from
        // the deck, and in a short bore no higher than the straight run between the portals, so it meets
        // the other one
        let over = |i: usize| {
            [deck_s.map(|d| d - i as i32 / TUNNEL_RAMP_RUN), deck_e.map(|d| d - (last - i) as i32 / TUNNEL_RAMP_RUN)]
                .into_iter()
                .flatten()
                .max()
                .map_or(r.terrain[i], |held| r.terrain[i].max(if long { held } else { held.min(grade(i)) }))
        };
        // as deep as cover asks, ramping down from whatever the portals reached
        let mut ys: Vec<i32> = (0..n)
            .map(|i| {
                let desired = floor[i];
                let ramp_s = if internal_s { i32::MIN } else { gs - start_drop - i as i32 / TUNNEL_RAMP_RUN };
                let ramp_e = if internal_e { i32::MIN } else { ge - end_drop - (last - i) as i32 / TUNNEL_RAMP_RUN };
                desired.max(ramp_s.max(ramp_e)).min(over(i))
            })
            .collect();
        let cap = |ys: &mut Vec<i32>| {
            for i in 1..n {
                ys[i] = ys[i].min(ys[i - 1] + TUNNEL_RAMP_STEP);
            }
            for i in (0..last).rev() {
                ys[i] = ys[i].min(ys[i + 1] + TUNNEL_RAMP_STEP);
            }
        };
        cap(&mut ys);
        // one valley, not several, in a short bore: the lowest point of a long one is not a ring's to know
        if let Some(m) = (0..n).filter(|_| !long).min_by_key(|&i| ys[i]) {
            for i in 1..=m {
                ys[i] = ys[i].min(ys[i - 1]);
            }
            for i in (m..last).rev() {
                ys[i] = ys[i].min(ys[i + 1]);
            }
        }
        // a short gap in the roof closes, so a wiggle of the terrain opens no skylight
        let mut covered: Vec<bool> = (0..n).map(|i| cover[i] - ys[i] >= roof).collect();
        let close = (2 * r.half + 3) as usize;
        let mut deepened = false;
        let mut i = 0;
        while i < n {
            if covered[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < n && !covered[i] {
                i += 1;
            }
            if start == 0 || i == n || i - start >= close {
                continue;
            }
            for j in start..i {
                if cover[j] - roof < ys[j] {
                    ys[j] = cover[j] - roof;
                    deepened = true;
                }
            }
        }
        if deepened {
            cap(&mut ys);
            covered = (0..n).map(|i| cover[i] - ys[i] >= roof).collect();
        }
        out.push((q, ys, covered));
    }
    out
}

/// For each cell `i`, the least over the cells `j` within `w` of it of
/// `room[j] + grade · |i − j|`: the highest floor at most `room` everywhere
/// that rises no faster than `grade` a cell, leaning on nothing further than
/// `w`. Two passes, each a sliding minimum, so a cell costs the same whatever `w`.
fn envelope(room: &[i32], grade: f64, w: usize) -> Vec<f64> {
    let n = room.len();
    let mut out = vec![f64::INFINITY; n];
    for reverse in [false, true] {
        // along the pass, `t` from where it starts: the least of room[j] − grade·t_j over the window, plus grade·t
        let mut window: VecDeque<(usize, f64)> = VecDeque::new();
        for t in 0..n {
            let i = if reverse { n - 1 - t } else { t };
            let v = f64::from(room[i]) - grade * t as f64;
            while window.back().is_some_and(|&(_, b)| b >= v) {
                window.pop_back();
            }
            window.push_back((t, v));
            while window.front().is_some_and(|&(s, _)| s + w < t) {
                window.pop_front();
            }
            let least = window.front().map_or(v, |&(_, b)| b);
            out[i] = out[i].min(least + grade * t as f64);
        }
    }
    out
}

/// What a bore's cell is: under cover, open to the sky, or held over the
/// ground by a deck's portal.
#[derive(Clone, Copy, PartialEq)]
enum Bore {
    Covered,
    Open,
    Raised,
}

/// Stretches of cells alike, each reaching to the next's first cell so the
/// line goes on unbroken, with what they are.
fn split_by<T: Copy + PartialEq>(cells: &[T]) -> Vec<([usize; 2], T)> {
    let mut out = Vec::new();
    let mut from = 0;
    for i in 1..=cells.len() {
        if i == cells.len() || cells[i] != cells[from] {
            out.push(([from, i.min(cells.len() - 1)], cells[from]));
            from = i;
        }
    }
    out
}

/// Cells `a..=b` of a run as a line of `form` at heights `ys`: the piece's
/// vertices and where the slope bends, no more.
fn part_of(r: &Run, ys: &[i32], [a, b]: [usize; 2], form: Form, clear: u8) -> (Line, Drawn) {
    let step = mer_from_u(CELL_U);
    let along = |k: usize| ((r.k0 + k) as f64 * step).min(r.piece.way.length());
    let mut keep: Vec<(f64, [f64; 2], f64)> = Vec::new();
    let mut anchor = a;
    for k in a..=b {
        let straight = k > a && k < b && {
            let (h0, h1, h2) = (i64::from(ys[anchor]), i64::from(ys[k]), i64::from(ys[k + 1]));
            (h1 - h0) * (k + 1 - anchor) as i64 == (h2 - h0) * (k - anchor) as i64
        };
        if !straight {
            anchor = k;
            keep.push((along(k), r.at[k], f64::from(ys[k])));
        }
    }
    let mut distance = 0.0;
    for seg in r.piece.way.points.windows(2) {
        distance += libm::hypot(seg[1][0] - seg[0][0], seg[1][1] - seg[0][1]);
        if distance > along(a) && distance < along(b) {
            let i = ((distance / step).floor() as usize).saturating_sub(r.k0).clamp(a, b - 1);
            let z = f64::from(ys[i]) + f64::from(ys[i + 1] - ys[i]) * (distance - along(i)) / step;
            keep.push((distance, [seg[1][0], seg[1][1]], round(z)));
        }
    }
    keep.sort_by(|x, y| x.0.total_cmp(&y.0));
    keep.dedup_by(|x, y| x.0 == y.0);
    let fraction = |d: f64| r.piece.way.span[0] + d / r.piece.way.segment_length;
    let points: Vec<[f64; 3]> = keep.iter().map(|(_, p, z)| [p[0], p[1], *z]).collect();
    let last = r.at.len() - 1;
    (
        Line {
            subtype: r.piece.way.subtype.clone(),
            class: r.piece.way.class.clone(),
            width_u: r.width_u,
            form,
            coarsest: super::coarsest(&points, [!(a == 0 && r.clipped[0]), !(b == last && r.clipped[1])]),
            points,
            clear,
        },
        Drawn {
            gers_id: r.piece.way.gers_id.clone(),
            range: [fraction(along(a)), fraction(along(b))],
        },
    )
}

#[cfg(test)]
#[path = "arnis.test.rs"]
mod tests;
