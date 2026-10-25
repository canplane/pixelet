// pxl-geo-bake/separation/solve.rs

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

//! A tile's bridges and tunnels, solved: what it reads, the structures in
//! scope, the stretches of them it sees (`Stretch`), and the lines it keeps; the
//! bridges are `bridge`'s, the tunnels `tunnel`'s. It began as louis-e/arnis `c96872e`
//! ported, read for Overture's fields where Arnis reads OSM's tags, its widths
//! the ground's, and its numbers, metres (a block a metre upright on the
//! Earth, as its celestial bodies say), taken into the tile's cells, half to
//! even. What is read is the tile and as much of its 1-ring as the structures
//! touching it reach, and no further.
//!
//! The 1-ring is the most a tile may consult, not what it must: nothing is
//! read past it, and nothing reads all of it where less will do. A tile draws
//! only itself (`terrain/pipeline/separation.rs` keeps what lies in it), so
//! what it computes past what bears on its own cells is spent for nothing. Two
//! rules follow, and a change here or in `bridge` or `tunnel` keeps both:
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
//!   (a long bore, `tunnel::road_bores`; a deck, `bridge::deck_block`).
//!
//! Arnis is where this solve started, not a bound on it: where its answer
//! needs a structure whole, the ring wins and the answer is reworked.

mod bridge;
mod tunnel;

use fnv::{FnvHashMap, FnvHashSet};
use pxl_spec::procedural::Frame;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use std::collections::{BTreeMap, BTreeSet};

use crate::terrain::CELL_U;
use pxl_spec::bake::separation::{footprint as line_reach, Form};
use pxl_spec::space::coord::adapter::epsg3857::{get_tile_bbox, mer_from_u, u_from_mer};
use pxl_spec_core::ways;
use xpute_kit::math::scalar::round;

use crate::segment::{End, Part, PartId};
use crate::separation::elevation::{sample_cells, tile_of};
use crate::separation::piece::{piece_of, Piece};
use crate::separation::source::{portals_in, ring, Store};
use crate::separation::way::{Polyline, Way};
use crate::separation::{Drawn, Line, Reader, Solved};
use crate::terrain::pipeline::overture::width::width_u;
use crate::wkb::wkb_to_geometries;

/// The tile's scale: Arnis's metres in leaf cells (pxl-spec-core
/// `cells_of_m`), at the tile's `Frame::per_m`.
#[derive(Clone, Copy)]
struct Scale {
    per_m: f32,
}

impl Scale {
    fn c(self, m: f64) -> i32 {
        pxl_spec_core::cells_of_m(m as f32, self.per_m)
    }
}

/// What the search has read: the ring's tiles it reached, their ground, every
/// way on them, their water, and their structures' pieces.
struct Ring {
    tiles: BTreeSet<[i32; 2]>,
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
fn is_road_tunnel(piece: &Piece) -> bool {
    piece.form == Form::Tunnel && piece.way.subtype != "rail"
}

/// A stretch of a piece inside the ring: what Arnis takes for a way, its
/// cells a cell apart.
struct Stretch {
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

impl Stretch {
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
    let scale = Scale {
        per_m: Frame::of(Tile::new(LEAF_ZOOM, tile)).per_m(),
    };
    let mut src = Ring {
        tiles: tiles.into_iter().collect(),
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
    let reach_of = |level: i64| tunnel::approach_reach(level, scale);
    let off = |p: [f64; 2]| {
        let d = [0, 1].map(|k| (own[0][k] - p[k]).max(p[k] - own[1][k]).max(0.0));
        libm::hypot(d[0], d[1])
    };
    // and never past the ring, whatever the reach comes to: the query is cut to the ring's box, as `load`
    // refuses a tile past it
    let asked = grown(own, reach_of(tunnel::DEEPEST));
    let asked = [[asked[0][0].max(lo[0]), asked[0][1].max(lo[1])], [asked[1][0].min(hi[0]), asked[1][1].min(hi[1])]];
    for p in portals_in(r.client, asked)? {
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
        is_road_tunnel(&s.piece) && [p[0], p[p.len() - 1]].iter().any(|e| off([e[0], e[1]]) <= reach_of(s.piece.way.tags.level.unwrap_or(0)))
    };
    let mut queue: Vec<PartId> = src.pieces.iter().filter(|(_, s)| s.piece.touches(own) || portal_near(s)).map(|(id, _)| id.clone()).collect();
    // a tile no structure touches nor sinks into has nothing to solve
    if queue.is_empty() {
        return Ok(Solved::default());
    }
    let reach = mer_from_u(f64::from(scale.c(bridge::DUAL_CARRIAGEWAY_MAX_DISTANCE_M) + 16) * CELL_U);
    // each piece is looked at once, against the pieces read so far; a piece a later read brings in
    // is held against what was looked at before it (`seen`), so no pass is repeated
    let lies_on = |o: &Piece, ends: &FnvHashSet<End>| (o.form == Form::Bridge || is_road_tunnel(o)) && o.way.points.iter().any(|p| ends.contains(&End::at([p[0], p[1]])));
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
        if is_road_tunnel(&s.piece) {
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
    let stretches = stretches_of(&src, &scope, lo);
    let mut lines: Vec<(Line, Drawn)> = Vec::new();

    // ---- road bridges, lower first; rail bridges over them; then the tunnels ----
    let decks = bridge::decks(&src, &stretches, [lo, hi], scale, &mut lines);
    tunnel::bores(&src, &stretches, &decks, scale, &mut lines);
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

impl Ring {
    /// Whether a point lies in the ring: past it nothing of a way is used.
    fn inside(&self, p: [f64; 2]) -> bool {
        self.tiles.contains(&tile_of(p))
    }

    /// Whether a point's ground is held: its tile of the ring was read.
    fn held(&self, p: [f64; 2]) -> bool {
        self.loaded.contains(&tile_of(p))
    }

    /// Reads a tile of the ring once; a tile past it is never read.
    fn load(&mut self, r: &mut Reader<impl Store>, p: [i32; 2]) -> Result<(), String> {
        if !self.tiles.contains(&p) || self.loaded.contains(&p) {
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
            .filter(|(_, p)| self.tiles.contains(&tile_of(*p)))
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
fn stretches_of(src: &Ring, scope: &BTreeSet<PartId>, origin: [f64; 2]) -> Vec<Stretch> {
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
            // a stretch's end the ring cut (`End::Ring`) marks where this tile stops looking, never an
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
            out.push(Stretch {
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

/// The cells about one whose centers lie within `reach` cells of its own, as
/// a way covers them (pxl-spec-core `covers`, here its distance to its own).
fn disk(reach: f32) -> Vec<[i64; 2]> {
    let r = libm::ceilf(reach) as i64;
    (-r..=r)
        .flat_map(|dz| (-r..=r).map(move |dx| [dx, dz]))
        .filter(|&[dx, dz]| libm::hypotf(dx as f32, dz as f32) <= reach)
        .collect()
}

/// A way's width, in u.
fn way_width(row: &Part) -> f64 {
    width_u(&row.subtype, row.class.as_deref(), row.tags.role)
}

/// A point in DB coordinates as cells from the world's corner: the same bits on every tile, where a
/// ring's own cells (`Stretch::cell`), taken from its corner, differ in their last bits from one tile's to
/// the next's, enough to move what lies on a window's edge.
fn absolute(p: [f64; 2]) -> [f64; 2] {
    [u_from_mer(p[0]) / CELL_U, u_from_mer(p[1]) / CELL_U]
}

/// Cells `a..=b` of a stretch as a line of `form` at heights `ys`: the piece's
/// vertices and where the slope bends, no more.
fn line_of(r: &Stretch, ys: &[i32], [a, b]: [usize; 2], form: Form, clear: u8) -> (Line, Drawn) {
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
