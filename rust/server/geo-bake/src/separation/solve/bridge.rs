// pxl-geo-bake/separation/solve/bridge.rs

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

//! A tile's decks: a road bridge's, the least concave line over what it must
//! meet or clear, in panels along its way (`DECK_PANEL`), and a rail bridge's,
//! riding a road deck or one level a viaduct with ramps (Arnis's bridges,
//! highways and railways, without the blocks it places; the solve's rules,
//! `super`).

use fnv::{FnvHashMap, FnvHashSet};
use std::collections::BTreeMap;

use crate::terrain::CELL_U;
use pxl_spec::bake::separation::Form;
use pxl_spec::space::coord::adapter::epsg3857::{mer_from_u, u_from_mer};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec_core::param::way::STACKED_DECK_HEADROOM_M;
use pxl_spec_core::way;
use xpute_kit::math::scalar::round;

use crate::segment::{End, PartId};
use crate::separation::elevation::sample_cells;
use crate::separation::way::{Polyline, Way};
use crate::separation::{Drawn, Line};
use crate::terrain::pipeline::overture::policy::waterway_width;

use super::*;

// ---- Arnis's numbers for a deck, in metres or as ratios, under its names ----
const LAYER_HEIGHT_STEP_M: f64 = 6.0;
const FLAT_TERRAIN_DIP_THRESHOLD_M: f64 = 4.0;
const SHORT_BRIDGE_LENGTH_M: f64 = 30.0;
pub(super) const DUAL_CARRIAGEWAY_MAX_DIST_M: f64 = 12.0;
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
fn is_non_vehicular(class: &str) -> bool {
    matches!(
        class,
        "footway" | "pedestrian" | "path" | "cycleway" | "bridleway" | "steps"
    )
}

/// Arnis's `effective_layer`: the layer, never below 0; a bridge untagged is 1.
fn effective_layer(r: &Stretch) -> i64 {
    r.piece
        .way
        .tags
        .level
        .map(|l| l.max(0))
        .unwrap_or(i64::from(r.piece.form == Form::Bridge))
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

    fn of(src: &Ring, [origin, far]: [[f64; 2]; 2], scale: Scale) -> Obstacles {
        let cell = |p: [f64; 2]| {
            [
                u_from_mer(p[0] - origin[0]) / CELL_U,
                u_from_mer(p[1] - origin[1]) / CELL_U,
            ]
        };
        let mut out = Obstacles {
            list: Vec::new(),
            buckets: FnvHashMap::default(),
            bucket: f64::from(scale.c(OBSTACLE_GRID_M).max(1)),
        };
        let add =
            |out: &mut Obstacles, points: &[[f64; 2]], ends: &[End], width: f32, headroom: i32| {
                let at_end = [points[0], points[points.len() - 1]];
                let nodes: Vec<End> = points
                    .iter()
                    .filter(|&&p| src.inside(p))
                    .map(|&p| End::at(p))
                    .chain(
                        ends.iter()
                            .zip(at_end)
                            .filter(|(_, p)| src.inside(*p))
                            .map(|(e, _)| e.clone()),
                    )
                    .collect();
                for pair in points.windows(2) {
                    let Some((a, b)) = clip(pair[0], pair[1], [origin, far]) else {
                        continue;
                    };
                    let pair = [cell(a), cell(b)];
                    let i = out.list.len();
                    out.list.push(Obstacle {
                        a: pair[0],
                        b: pair[1],
                        width,
                        headroom,
                        nodes: nodes.clone(),
                    });
                    let r = f64::from(way::reach(width));
                    let k = |v: f64| (v / out.bucket).floor() as i64;
                    for bx in k(pair[0][0].min(pair[1][0]) - r)..=k(pair[0][0].max(pair[1][0]) + r)
                    {
                        for by in
                            k(pair[0][1].min(pair[1][1]) - r)..=k(pair[0][1].max(pair[1][1]) + r)
                        {
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
                    let headroom = if is_non_vehicular(class) || class == "track" {
                        PATH_HEADROOM_M
                    } else {
                        ROAD_HEADROOM_M
                    };
                    add(
                        &mut out,
                        &way.line.points,
                        &row.ends,
                        (way_width(row) / CELL_U) as f32,
                        scale.c(headroom),
                    );
                }
                "rail" if !row.is_subway() => {
                    let headroom = if row.class.as_deref() == Some("tram") {
                        ROAD_HEADROOM_M
                    } else {
                        RAIL_HEADROOM_M
                    };
                    add(
                        &mut out,
                        &way.line.points,
                        &row.ends,
                        (way_width(row) / CELL_U) as f32,
                        scale.c(headroom),
                    );
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
            add(
                &mut out,
                points,
                &[],
                (waterway_width(class).unwrap_or(0.0) / CELL_U) as f32,
                scale.c(headroom),
            );
        }
        out
    }

    fn at(&self, p: [f64; 2]) -> impl Iterator<Item = &Obstacle> {
        let k = (
            (p[0] / self.bucket).floor() as i64,
            (p[1] / self.bucket).floor() as i64,
        );
        self.buckets
            .get(&k)
            .into_iter()
            .flatten()
            .map(|&i| &self.list[i])
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
pub(super) type Laid = FnvHashMap<(i64, i64), (i32, i64)>;

pub(super) struct Decks {
    pub(super) ys: Vec<(usize, Vec<i32>)>,
    pub(super) laid: Laid,
    /// The ways a bridge's ramp took, kept from a tunnel's approach.
    pub(super) ramp_ways: FnvHashSet<PartId>,
}

fn cell_key(c: [f64; 2]) -> (i64, i64) {
    (c[0].floor() as i64, c[1].floor() as i64)
}

/// The way's heading end to end, whole, not as far as the ring cut it.
fn heading_of(r: &Stretch) -> Option<f64> {
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
    diff <= DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG
        || (180.0 - diff).abs() <= DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG
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

fn lateral(p: [f64; 2], r: &Stretch) -> f64 {
    r.whole
        .points
        .windows(2)
        .map(|w| dist_to_segment(p, w[0], w[1]))
        .fold(f64::INFINITY, f64::min)
}

/// Arnis `side_by_side`: parallel decks forming one, across layers only when they touch. Weighed
/// on the ways whole, as Arnis weighs an OSM way, so two tiles that see both decide alike wherever
/// their rings cut them.
fn side_by_side(a: &Stretch, b: &Stretch, scale: Scale) -> bool {
    let (Some(ha), Some(hb)) = (heading_of(a), heading_of(b)) else {
        return false;
    };
    if !parallel(ha, hb) {
        return false;
    }
    let same = effective_layer(a) == effective_layer(b);
    // their edges meet where each way's cover ends (pxl-spec-core `reach`)
    let reach = |r: &Stretch| f64::from(way::reach((r.width_u / CELL_U) as f32));
    let touching = reach(a) + reach(b) + f64::from(scale.c(SIDE_DECK_EDGE_GAP_M));
    let max = if same {
        touching.max(f64::from(scale.c(DUAL_CARRIAGEWAY_MAX_DIST_M)))
    } else {
        touching
    };
    let (short, long) = if a.whole.length() <= b.whole.length() {
        (a, b)
    } else {
        (b, a)
    };
    let offsets: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0]
        .iter()
        .map(|f| lateral(short.whole.at(f * short.whole.length()), long))
        .collect();
    let alongside = offsets.iter().filter(|&&d| d <= max).count();
    if same {
        alongside * 2 > offsets.len()
    } else {
        alongside == offsets.len() && offsets.iter().sum::<f64>() / offsets.len() as f64 >= 1.5
    }
}

struct UnionFind(Vec<usize>);

impl UnionFind {
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
    let (Some(&first), Some(&last)) = (us.first(), us.last()) else {
        return true;
    };
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
    (0..=n)
        .map(|k| line.at((k as f64 * step).min(line.length())))
        .collect()
}

fn ramp_len(total: f64, scale: Scale) -> f64 {
    (total * DECK_RAMP_FRACTION).clamp(
        f64::from(scale.c(DECK_RAMP_M[0])),
        f64::from(scale.c(DECK_RAMP_M[1])),
    )
}

// ---- a deck, leaning on nothing further than a tile ----

/// A deck is solved in panels of `DECK_PANEL` cells along its way whole, each from what lies about
/// the panel's middle, and a cell between two middles is the blend of the two; the panels fall where
/// they fall on the way, wherever a ring cuts it (the module's rules). What the source gives, the
/// ground, the ways and waters a deck clears, its ends, is weighed within `DECK_WINDOW`. What another
/// deck sets, the deck it clears and the joint it meets, leans in turn on that deck's own window, so
/// it is weighed only within `DECK_NEAR`, the cell's own included, and the reach adds up over the
/// levels stacked (a deck over a deck, a track riding it, a bore opening on it): `DECK_LEVELS` of them
/// fit in a tile, which every ring holds about what its tile draws. Arnis solves a structure as one,
/// over all of it, which no ring can; a deck no longer than a window comes out much as it would.
const DECK_PANEL: usize = 16;
const DECK_WINDOW: usize = 384;
const DECK_NEAR: usize = 16;
const DECK_LEVELS: usize = 4;
const _: () = assert!(
    DECK_LEVELS * DECK_PANEL + (DECK_LEVELS - 1) * DECK_NEAR + DECK_WINDOW <= TILE_CELLS as usize
);

/// What a group's decks must meet or clear, each where it lies, in the ring's cells.
#[derive(Default)]
struct Needs {
    /// The ground under each cell, and its member.
    grounds: Vec<([f64; 2], i32, usize)>,
    /// What a cell must clear, that capped so the deck can still come down to every anchor, and
    /// whether another deck sets it (a deck under it to clear).
    clears: Vec<([f64; 2], i32, f64, bool)>,
    /// An end the deck comes down to: the ground, or a deck it joins (`true`); and its member.
    anchors: Vec<([f64; 2], i32, bool, usize)>,
    /// A free end, which stays up for the ramp that carries the way on.
    frees: Vec<[f64; 2]>,
    /// Every end that is the structure's own.
    bounds: Vec<[f64; 2]>,
}

/// A member's way whole: where it starts and its heading, along which its deck is solved, whether
/// it keeps to that heading, its layer and its length in cells.
struct Member {
    origin: [f64; 2],
    dir: [f64; 2],
    follows: bool,
    level: i64,
    length: f64,
}

/// A panel's deck: the least concave line over what it must meet, along its member's heading, or
/// one level where a way about it winds off its own.
enum Panel {
    Hull(Vec<(f64, f64)>),
    Level(i32),
}

impl Panel {
    fn at(&self, u: f64) -> f64 {
        match self {
            Panel::Hull(hull) => hull_at(hull, u),
            Panel::Level(y) => f64::from(*y),
        }
    }
}

/// Stretch `r`'s deck cell by cell, member `m` of its group (`DECK_PANEL`).
fn deck_of(r: &Stretch, m: usize, members: &[Member], needs: &Needs, scale: Scale) -> Vec<i32> {
    let me = &members[m];
    let mut panels: FnvHashMap<usize, Panel> = FnvHashMap::default();
    (0..r.cell.len())
        .map(|i| {
            // its place along the way whole, which no ring moves
            let a = r.k0 + i;
            let (k, t) = (a / DECK_PANEL, (a % DECK_PANEL) as f64 / DECK_PANEL as f64);
            let u = along(me, absolute(r.at[i]));
            let mut at = |k: usize| {
                panels
                    .entry(k)
                    .or_insert_with(|| deck_panel(r, k, m, members, needs, scale))
                    .at(u)
            };
            let y = if t > 0.0 {
                at(k) * (1.0 - t) + at(k + 1) * t
            } else {
                at(k)
            };
            round(y) as i32
        })
        .collect()
}

/// Where `p` lies along a member's heading.
fn along(me: &Member, p: [f64; 2]) -> f64 {
    (p[0] - me.origin[0]) * me.dir[0] + (p[1] - me.origin[1]) * me.dir[1]
}

/// Panel `k` of stretch `r`'s deck, from what its group must meet about its middle: as Arnis's structure,
/// but over the windows (`DECK_WINDOW`, `DECK_NEAR`), not the whole. The ground floors the deck and is
/// no point of its line, so a deck with one thing to clear in sight holds its height over it rather
/// than dipping to the ground at the window's edge; where an end is in sight, it comes down to it.
fn deck_panel(
    r: &Stretch,
    k: usize,
    m: usize,
    members: &[Member],
    needs: &Needs,
    scale: Scale,
) -> Panel {
    let me = &members[m];
    let c = r.whole.at(((k * DECK_PANEL) as f64).min(r.whole.length()));
    let within = |p: &[f64; 2], d: usize| libm::hypot(p[0] - c[0], p[1] - c[1]) <= d as f64;
    // what the source gives within the window, what another deck sets within the near
    let reach = |set_by_deck: bool| if set_by_deck { DECK_NEAR } else { DECK_WINDOW };
    let grounds: Vec<&([f64; 2], i32, usize)> = needs
        .grounds
        .iter()
        .filter(|g| within(&g.0, DECK_WINDOW))
        .collect();
    let anchors: Vec<&([f64; 2], i32, bool, usize)> = needs
        .anchors
        .iter()
        .filter(|a| within(&a.0, reach(a.2)))
        .collect();
    let clears: Vec<&([f64; 2], i32, f64, bool)> = needs
        .clears
        .iter()
        .filter(|n| within(&n.0, reach(n.3)))
        .collect();
    let (Some(tmax), Some(tmin)) = (
        grounds.iter().map(|g| g.1).max(),
        grounds.iter().map(|g| g.1).min(),
    ) else {
        return Panel::Level(0);
    };
    let mut about = vec![false; members.len()];
    for g in &grounds {
        about[g.2] = true;
    }
    let about = || {
        members
            .iter()
            .zip(&about)
            .filter(|(_, &a)| a)
            .map(|(m, _)| m)
    };
    // a long flat span rises to its layer
    let total: f64 = about().map(|m| m.length).sum();
    let flat = needs.bounds.iter().any(|b| within(b, DECK_WINDOW))
        && tmax - tmin < scale.c(FLAT_TERRAIN_DIP_THRESHOLD_M)
        && total >= f64::from(scale.c(SHORT_BRIDGE_LENGTH_M));
    let clearance = if flat {
        about().map(|m| m.level).max().unwrap_or(0) as i32 * scale.c(LAYER_HEIGHT_STEP_M)
    } else {
        0
    };
    let plateau = (clearance > 0).then_some(tmax + clearance);
    if !about().all(|m| m.follows) {
        return Panel::Level(
            clears
                .iter()
                .map(|n| n.1)
                .chain(plateau)
                .fold(tmax, i32::max),
        );
    }
    let raised = anchors
        .iter()
        .filter(|a| a.2)
        .map(|a| a.1)
        .chain(clears.iter().map(|n| round(n.2) as i32))
        .chain(plateau)
        .fold(tmax, i32::max);
    let u = |p: [f64; 2]| along(me, p);
    // a joint holds every deck about it; an end at the ground only its own way's, which ramps down to it,
    // where Arnis's one line over the whole passes over an end in the middle of a structure
    let mut pts: Vec<(f64, f64)> = anchors
        .iter()
        .filter(|a| a.2 || a.3 == m)
        .map(|a| (u(a.0), f64::from(a.1)))
        .collect();
    pts.extend(clears.iter().map(|n| (u(n.0), n.2)));
    if let Some(top) = plateau {
        let (lo, hi) = grounds
            .iter()
            .map(|g| u(g.0))
            .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
        let span = (hi - lo).max(0.0);
        let ramp = ramp_len(span, scale).min(span / 2.0);
        let mut placed = false;
        for g in &grounds {
            if anchors
                .iter()
                .all(|a| libm::hypot(g.0[0] - a.0[0], g.0[1] - a.0[1]) >= ramp)
            {
                pts.push((u(g.0), f64::from(top)));
                placed = true;
            }
        }
        if !placed {
            pts.push(((lo + hi) / 2.0, f64::from(top)));
        }
    }
    pts.extend(
        needs
            .frees
            .iter()
            .filter(|f| within(f, DECK_WINDOW))
            .map(|f| (u(*f), f64::from(raised))),
    );
    let Some(top) = pts.iter().map(|p| p.1).reduce(f64::max) else {
        // nothing to meet or clear in sight: the deck stands on its highest ground about it
        return Panel::Level(tmax);
    };
    // where no end of its own way is in sight, the structure goes on past the window, and the deck
    // holds the highest it meets in sight rather than falling to something low at the window's edge
    // that a whole structure's line would pass over; an end in sight it comes down to
    let uc = u(c);
    let ends: Vec<f64> = anchors
        .iter()
        .filter(|a| !a.2 && a.3 == m)
        .map(|a| u(a.0))
        .collect();
    if !ends.iter().any(|&e| e < uc) {
        pts.push((uc - DECK_WINDOW as f64, top));
    }
    if !ends.iter().any(|&e| e > uc) {
        pts.push((uc + DECK_WINDOW as f64, top));
    }
    Panel::Hull(upper_hull(pts))
}

fn road_decks(
    src: &Ring,
    stretches: &[Stretch],
    obstacles: &Obstacles,
    scale: Scale,
    lines: &mut Vec<(Line, Drawn)>,
) -> Decks {
    let road: Vec<usize> = (0..stretches.len())
        .filter(|&q| stretches[q].piece.form == Form::Bridge && !stretches[q].rail())
        .collect();
    let mut uf = UnionFind((0..road.len()).collect());
    // shared ends at one layer, then decks side by side
    let mut by_node: BTreeMap<&End, Vec<usize>> = BTreeMap::new();
    for (i, &q) in road.iter().enumerate() {
        for e in &stretches[q].ends {
            by_node.entry(e).or_default().push(i);
        }
    }
    for members in by_node.values() {
        for (a, &i) in members.iter().enumerate() {
            for &j in &members[a + 1..] {
                if effective_layer(&stretches[road[i]]) == effective_layer(&stretches[road[j]]) {
                    uf.union(i, j);
                }
            }
        }
    }
    // two decks whose cells lie further apart than any deck beside another (`side_by_side`'s
    // farthest) are not weighed
    let boxes: Vec<[[f64; 2]; 2]> = road
        .iter()
        .map(|&q| cells_box(&stretches[q].whole.points))
        .collect();
    let reach = |q: usize| f64::from(way::reach((stretches[q].width_u / CELL_U) as f32));
    let farthest = |a: usize, b: usize| {
        (reach(a) + reach(b) + f64::from(scale.c(SIDE_DECK_EDGE_GAP_M)))
            .max(f64::from(scale.c(DUAL_CARRIAGEWAY_MAX_DIST_M)))
    };
    for i in 0..road.len() {
        for j in i + 1..road.len() {
            let apart = (0..2)
                .map(|k| (boxes[j][0][k] - boxes[i][1][k]).max(boxes[i][0][k] - boxes[j][1][k]))
                .fold(0.0, f64::max);
            if apart <= farthest(road[i], road[j])
                && uf.find(i) != uf.find(j)
                && side_by_side(&stretches[road[i]], &stretches[road[j]], scale)
            {
                uf.union(i, j);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &r) in road.iter().enumerate() {
        groups.entry(uf.find(i)).or_default().push(r);
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().collect();
    groups.sort_by_key(|g| {
        (
            g.iter()
                .map(|&q| effective_layer(&stretches[q]))
                .max()
                .unwrap_or(0),
            stretches[g[0]].piece.id(),
        )
    });

    let mut bridge_nodes: FnvHashMap<&End, Vec<usize>> = FnvHashMap::default();
    for q in (0..stretches.len()).filter(|&q| stretches[q].piece.form == Form::Bridge) {
        for e in &stretches[q].ends {
            bridge_nodes.entry(e).or_default().push(q);
        }
    }
    let mut laid: Laid = FnvHashMap::default();
    let mut joints: FnvHashMap<End, i32> = FnvHashMap::default();
    let mut claimed: FnvHashSet<PartId> = FnvHashSet::default();
    let mut out = Vec::new();
    for g in &groups {
        let mut node_members: FnvHashMap<&End, usize> = FnvHashMap::default();
        let mut group_nodes: FnvHashSet<&End> = FnvHashSet::default();
        for &q in g {
            let seen: FnvHashSet<&End> = stretches[q].nodes.iter().collect();
            for n in &seen {
                *node_members.entry(n).or_default() += 1;
            }
            group_nodes.extend(seen);
        }
        // an end the ring cut is none (the module's rules): no anchor, no free end, no ramp
        let is_boundary =
            |n: &End| !matches!(n, End::Ring(..)) && node_members.get(n).copied().unwrap_or(0) <= 1;
        let joins_other = |n: &End| {
            bridge_nodes
                .get(n)
                .is_some_and(|qs| qs.iter().any(|q| !g.contains(q)))
        };

        // ramps: a way at a free end, raised and no bridge, carries the deck down
        let mut ramp_ends: FnvHashSet<&End> = FnvHashSet::default();
        let mut pending: Vec<(PartId, Vec<[f64; 2]>, End, i32)> = Vec::new();
        for &q in g {
            for e in &stretches[q].ends {
                if !is_boundary(e) {
                    continue;
                }
                for part in src.way_ends.get(e).into_iter().flatten() {
                    let way = &src.ways[part];
                    // a road's alone, never indoors (Arnis `is_ramp_candidate`)
                    let t = way.row.tags;
                    if way.row.subtype != "road"
                        || t.bridge
                        || t.tunnel
                        || t.indoor
                        || !t.raised
                        || claimed.contains(part)
                    {
                        continue;
                    }
                    let Some((line, far_end)) = way_from(way, e) else {
                        continue;
                    };
                    let cells = cells_along(&line);
                    // a ramp that leaves the ring is none: its far end is not known
                    if group_nodes.contains(far_end) || !cells.iter().all(|&p| src.inside(p)) {
                        continue;
                    }
                    let Ok(ground) = sample_cells(&src.ground, &[line[line.len() - 1]]) else {
                        continue;
                    };
                    pending.push((part.clone(), cells, e.clone(), ground[0]));
                    claimed.insert(part.clone());
                    ramp_ends.insert(e);
                }
            }
        }
        let anchors_end = |n: &End| is_boundary(n) && !ramp_ends.contains(n) && !joins_other(n);

        // what the deck must meet or clear, each where it lies, in the ring's cells
        let mut needs = Needs::default();
        for (m, &q) in g.iter().enumerate() {
            let r = &stretches[q];
            let last = r.cell.len() - 1;
            for (e, i) in [(&r.ends[0], 0), (&r.ends[1], last)] {
                if is_boundary(e) {
                    needs.bounds.push(absolute(r.at[i]));
                }
                if anchors_end(e) {
                    needs
                        .anchors
                        .push((absolute(r.at[i]), r.terrain[i], false, m));
                } else if let Some(&y) = joints.get(e) {
                    needs.anchors.push((absolute(r.at[i]), y, true, m));
                } else if is_boundary(e) {
                    needs.frees.push(absolute(r.at[i]));
                }
            }
        }
        // a clearance is capped so the deck can still come down to every anchor
        // an end at the ground caps from wherever it lies; a joint, another deck's, from near (`DECK_NEAR`)
        let reachable = |p: [f64; 2]| {
            needs
                .anchors
                .iter()
                .map(|&(a, ya, joint, _)| (libm::hypot(p[0] - a[0], p[1] - a[1]), ya, joint))
                .filter(|&(d, _, joint)| !joint || d <= DECK_NEAR as f64)
                .map(|(d, ya, _)| f64::from(ya) + d * MAX_CLEARANCE_SLOPE)
                .fold(f64::INFINITY, f64::min)
        };
        let mut clears = Vec::new();
        for (m, &q) in g.iter().enumerate() {
            let r = &stretches[q];
            let layer = effective_layer(r);
            for (i, &c) in r.cell.iter().enumerate() {
                let (at, ground) = (absolute(r.at[i]), r.terrain[i]);
                needs.grounds.push((at, ground, m));
                let mut need = ground;
                let (p, n) = (
                    r.cell[i.saturating_sub(1)],
                    r.cell[(i + 1).min(r.cell.len() - 1)],
                );
                for o in obstacles.at(c) {
                    // a way meeting the structure runs at its level, not under it
                    if way::covers(o.width, dist_to_segment(c, o.a, o.b) as f32)
                        && crosses([n[0] - p[0], n[1] - p[1]], o)
                        && !o.nodes.iter().any(|n| group_nodes.contains(n))
                    {
                        need = need.max(ground + o.headroom);
                    }
                }
                if need > ground {
                    clears.push((at, need, ground, false));
                }
                if let Some(&(lower, lower_layer)) = laid.get(&cell_key(c)) {
                    let over = lower + scale.c(f64::from(STACKED_DECK_HEADROOM_M));
                    if lower_layer < layer && over > ground {
                        clears.push((at, over, ground, true));
                    }
                }
            }
        }
        needs.clears = clears
            .into_iter()
            .map(|(c, need, ground, stacked)| {
                (
                    c,
                    need,
                    f64::from(need).min(reachable(c)).max(f64::from(ground)),
                    stacked,
                )
            })
            .collect();
        let members: Vec<Member> = g
            .iter()
            .map(|&q| {
                let w = &stretches[q].whole;
                let (s, e) = (w.points[0], w.points[w.points.len() - 1]);
                let d = libm::hypot(e[0] - s[0], e[1] - s[1]);
                let dir = if d > 0.0 {
                    [(e[0] - s[0]) / d, (e[1] - s[1]) / d]
                } else {
                    [1.0, 0.0]
                };
                let along = |c: &[f64; 2]| (c[0] - s[0]) * dir[0] + (c[1] - s[1]) * dir[1];
                Member {
                    origin: s,
                    dir,
                    // whether the way runs along its own heading, judged on the way whole
                    follows: d >= 1.0
                        && is_monotonic(
                            &w.points.iter().map(along).collect::<Vec<f64>>(),
                            f64::from(scale.c(PROFILE_MONOTONIC_TOLERANCE_M)),
                        ),
                    level: stretches[q].piece.way.tags.level.map_or(0, |l| l.max(0)),
                    length: w.length(),
                }
            })
            .collect();
        let mut ys: Vec<Vec<i32>> = g
            .iter()
            .enumerate()
            .map(|(m, &q)| deck_of(&stretches[q], m, &members, &needs, scale))
            .collect();

        // an end above the ground ramps down to it inside the member
        for (m, &q) in g.iter().enumerate() {
            let r = &stretches[q];
            let total = ys[m].len();
            let (gs, ge) = (r.terrain[0], r.terrain[total - 1]);
            let ramp_s = anchors_end(&r.ends[0]) && ys[m][0] > gs + 1;
            let ramp_e = anchors_end(&r.ends[1]) && ys[m][total - 1] > ge + 1;
            let cap = if ramp_s && ramp_e {
                (total / 2).max(1)
            } else {
                total - 1
            };
            // as long as the way whole asks, not as long as the stretch the ring left
            let len = (ramp_len(members[m].length, scale) as usize).clamp(1, cap);
            if ramp_s {
                let top = ys[m][len];
                for (k, y) in ys[m][..len].iter_mut().enumerate() {
                    *y = (*y).min(
                        round(f64::from(gs) + f64::from(top - gs) * k as f64 / len as f64) as i32,
                    );
                }
            }
            if ramp_e {
                let top = ys[m][total - 1 - len];
                for k in 0..len {
                    let y = &mut ys[m][total - 1 - k];
                    *y = (*y).min(
                        round(f64::from(ge) + f64::from(top - ge) * k as f64 / len as f64) as i32,
                    );
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
                    if stretches[q].ends[0] == e {
                        ys[m].first().copied()
                    } else if stretches[q].ends[1] == e {
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
                    .map(|(k, p)| {
                        [
                            p[0],
                            p[1],
                            round(
                                f64::from(deck_y) + f64::from(ground - deck_y) * k as f64 / denom,
                            ),
                        ]
                    })
                    .collect(),
                [true, true],
                way.row.span,
            ));
        }

        // what the structures after it meet and clear
        for (m, &q) in g.iter().enumerate() {
            let r = &stretches[q];
            for (e, y) in [(&r.ends[0], ys[m][0]), (&r.ends[1], ys[m][ys[m].len() - 1])] {
                let j = joints.entry(e.clone()).or_insert(y);
                *j = (*j).max(y);
            }
            let layer = effective_layer(r);
            let width = (r.width_u / CELL_U) as f32;
            let around = disk(way::reach(width));
            for (&c, &y) in r.cell.iter().zip(&ys[m]) {
                let (x, z) = cell_key(c);
                for &[dx, dz] in &around {
                    let e = laid.entry((x + dx, z + dz)).or_insert((y, layer));
                    if y > e.0 {
                        *e = (y, layer);
                    }
                }
            }
            out.push((q, std::mem::take(&mut ys[m])));
        }
    }
    Decks {
        ys: out,
        laid,
        ramp_ways: claimed,
    }
}

// ---- rail bridges: on a road deck, or one level a viaduct with ramps at its free ends ----

fn rail_decks(
    stretches: &[Stretch],
    obstacles: &Obstacles,
    laid: &Laid,
    scale: Scale,
) -> Vec<(usize, Vec<i32>)> {
    let rail: Vec<usize> = (0..stretches.len())
        .filter(|&q| stretches[q].piece.form == Form::Bridge && stretches[q].rail())
        .collect();
    let mut out = Vec::new();
    let mut ends_cnt: FnvHashMap<&End, usize> = FnvHashMap::default();
    for &q in &rail {
        for e in &stretches[q].ends {
            *ends_cnt.entry(e).or_default() += 1;
        }
    }
    // what each track cell rides: the nearest road deck of its layer within reach
    let reach = i64::from(scale.c(RAIL_ON_DECK_REACH_M));
    let on: Vec<Vec<Option<i32>>> = rail
        .iter()
        .map(|&q| {
            let layer = effective_layer(&stretches[q]);
            stretches[q]
                .cell
                .iter()
                .map(|&c| {
                    let (x, z) = cell_key(c);
                    let mut best: Option<(i64, i32)> = None;
                    for dx in -reach..=reach {
                        for dz in -reach..=reach {
                            if let Some(&(y, l)) = laid.get(&(x + dx, z + dz)) {
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
    let mut uf = UnionFind((0..rail.len()).collect());
    let mut at_end: FnvHashMap<(&End, i64), usize> = FnvHashMap::default();
    for (i, &q) in rail.iter().enumerate() {
        for e in &stretches[q].ends {
            match at_end.get(&(e, effective_layer(&stretches[q]))) {
                Some(&j) => uf.union(i, j),
                None => {
                    at_end.insert((e, effective_layer(&stretches[q])), i);
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
            let r = &stretches[rail[i]];
            let layer = effective_layer(r);
            for (j, &c) in r.cell.iter().enumerate() {
                let ground = r.terrain[j];
                let (mut floor, mut over) = (i32::MIN, i32::MIN);
                let (p, n) = (
                    r.cell[j.saturating_sub(1)],
                    r.cell[(j + 1).min(r.cell.len() - 1)],
                );
                for o in obstacles.at(c) {
                    if way::covers(o.width, dist_to_segment(c, o.a, o.b) as f32)
                        && crosses([n[0] - p[0], n[1] - p[1]], o)
                        && !o.nodes.iter().any(|n| r.nodes.contains(n))
                    {
                        floor = floor.max(ground + o.headroom);
                    }
                }
                if let Some(&(lower, lower_layer)) = laid.get(&cell_key(c)) {
                    if lower_layer < layer {
                        over = lower + scale.c(f64::from(STACKED_DECK_HEADROOM_M));
                    }
                }
                cells.push((absolute(r.at[j]), ground, floor, over, layer));
            }
        }
        let within =
            |c: [f64; 2], p: [f64; 2], d: usize| libm::hypot(p[0] - c[0], p[1] - c[1]) <= d as f64;
        let near = |c: [f64; 2], p: [f64; 2]| within(c, p, DECK_WINDOW);
        // a flat span lifts clear, a level more a layer; a valley span sits on its banks: over the
        // group's cells about the panel's middle, as a road deck's (`DECK_PANEL`), a deck under it
        // cleared only near
        let level_at = |c: [f64; 2]| {
            let (mut tmax, mut tmin, mut floor, mut layer) = (i32::MIN, i32::MAX, i32::MIN, 0);
            for &(p, ground, f, over, l) in cells.iter().filter(|x| near(c, x.0)) {
                let over = if within(c, p, DECK_NEAR) {
                    over
                } else {
                    i32::MIN
                };
                (tmax, tmin, floor, layer) = (
                    tmax.max(ground),
                    tmin.min(ground),
                    floor.max(f).max(over),
                    layer.max(l),
                );
            }
            if tmax == i32::MIN {
                return None;
            }
            Some(
                if tmax - tmin < scale.c(RAIL_BRIDGE_DIP_THRESHOLD_M) {
                    tmax + scale.c(RAIL_BRIDGE_FLAT_CLEARANCE_M)
                        + (layer as i32 - 1).max(0) * scale.c(LAYER_HEIGHT_STEP_M)
                } else {
                    tmax
                }
                .max(floor),
            )
        };
        for &i in &g {
            let r = &stretches[rail[i]];
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
            // panel's middle, and is one level a viaduct elsewhere
            let panel = |k: usize| {
                let c = r.whole.at(((k * DECK_PANEL) as f64).min(r.whole.length()));
                let mine: Vec<bool> = (0..total)
                    .filter(|&j| near(c, absolute(r.at[j])))
                    .map(|j| on[i][j].is_some())
                    .collect();
                let rides =
                    !mine.is_empty() && mine.iter().filter(|&&s| s).count() * 10 >= mine.len() * 9;
                (rides, if rides { None } else { level_at(c) })
            };
            let mut panels: FnvHashMap<usize, (bool, Option<i32>)> = FnvHashMap::default();
            let mut riding = vec![false; total];
            let deck: Vec<i32> = (0..total)
                .map(|j| {
                    let a = r.k0 + j;
                    let (k, t) = (a / DECK_PANEL, (a % DECK_PANEL) as f64 / DECK_PANEL as f64);
                    let ride = rode[j].unwrap_or(r.terrain[j]);
                    let mut at = |k: usize| {
                        let (rides, level) = *panels.entry(k).or_insert_with(|| panel(k));
                        (
                            rides,
                            if rides {
                                f64::from(ride)
                            } else {
                                f64::from(level.unwrap_or(ride))
                            },
                        )
                    };
                    let (rides, y) = at(k);
                    riding[j] = rides;
                    round(if t > 0.0 {
                        y * (1.0 - t) + at(k + 1).1 * t
                    } else {
                        y
                    }) as i32
                })
                .collect();
            // each way ramps from its free ends where it is a viaduct, no steeper than a cell a cell
            // (Arnis `generate_rail_bridge`); an end the ring cut is none, and the viaduct carries on there
            let internal = [
                ends_cnt[&r.ends[0]] > 1 || r.clipped[0] || riding[0],
                ends_cnt[&r.ends[1]] > 1 || r.clipped[1] || riding[last],
            ];
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
                .clamp(
                    scale.c(RAIL_BRIDGE_RAMP_M[0]) as usize,
                    scale.c(RAIL_BRIDGE_RAMP_M[1]) as usize,
                )
                .max(needed + 1);
            let denom = ramp.saturating_sub(1).max(1) as f64;
            let ys = (0..total)
                .map(|k| {
                    let s = if internal[0] {
                        deck[k]
                    } else {
                        round(f64::from(gs) + f64::from(deck[k] - gs) * (k as f64 / denom).min(1.0))
                            as i32
                    };
                    let e = if internal[1] {
                        deck[k]
                    } else {
                        round(
                            f64::from(ge)
                                + f64::from(deck[k] - ge) * ((last - k) as f64 / denom).min(1.0),
                        ) as i32
                    };
                    s.min(e).max(r.terrain[k])
                })
                .collect();
            out.push((rail[i], ys));
        }
    }
    out
}

/// The decks of the stretches in scope, road bridges lower first and rail bridges over them, as lines,
/// what a deck must clear read from the ring `ring`; and what a bore opening on one needs of them.
pub(super) fn decks(
    src: &Ring,
    stretches: &[Stretch],
    ring: [[f64; 2]; 2],
    scale: Scale,
    lines: &mut Vec<(Line, Drawn)>,
) -> Decks {
    // what a deck must clear, only where there is a deck
    let obstacles = if stretches.iter().any(|r| r.piece.form == Form::Bridge) {
        Obstacles::of(src, ring, scale)
    } else {
        Obstacles::none()
    };
    let solved = road_decks(src, stretches, &obstacles, scale, lines);
    for (q, ys) in &solved.ys {
        lines.push(line_of(
            &stretches[*q],
            ys,
            [0, ys.len() - 1],
            Form::Bridge,
            0,
        ));
    }
    for (q, ys) in rail_decks(stretches, &obstacles, &solved.laid, scale) {
        lines.push(line_of(
            &stretches[q],
            &ys,
            [0, ys.len() - 1],
            Form::Bridge,
            0,
        ));
    }
    solved
}

#[cfg(test)]
#[path = "bridge.test.rs"]
mod tests;
