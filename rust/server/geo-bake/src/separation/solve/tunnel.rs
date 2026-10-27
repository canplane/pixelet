// pxl-geo-bake/separation/solve/tunnel.rs

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

//! A tile's bores: a road tunnel as deep as its cover asks, reached by its
//! approach roads sinking into the portal, and a rail tunnel at a fixed depth
//! under the terrain (Arnis's highways and railways, without the blocks it
//! places; the solve's rules, `super`).

use fnv::{FnvHashMap, FnvHashSet};
use std::collections::{BTreeMap, VecDeque};

use crate::terrain::CELL_U;
use pxl_spec::bake::separation::Form;
use pxl_spec::space::coord::adapter::epsg3857::mer_from_u;
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec_core::way;
use xpute_kit::math::scalar::round;

use crate::segment::{End, PartId};
use crate::separation::elevation::{sample_cells, tile_of};
use crate::separation::{Drawn, Line};

use super::bridge::Decks;
use super::*;

// ---- Arnis's numbers for a bore, in metres, under its names ----
const TUNNEL_CEIL_OFFSET_M: f64 = 5.0;
const TUNNEL_COVER_DROP_M: f64 = 7.0;
const TUNNEL_ROOF_COVER_M: f64 = TUNNEL_CEIL_OFFSET_M + 1.0;
const TUNNEL_RAMP_STEP: i32 = 1;
const TUNNEL_RAMP_RUN: i32 = 3;
const TUNNEL_LAYER_DROP_M: f64 = 7.0;
const TUNNEL_MAX_LAYERS_BELOW: i64 = 8;
const TUNNEL_APPROACH_WAYS: usize = 8;
/// Ours, not Arnis's: the steepest a bore longer than a tile rises from the lowest floor near it
/// (`road_bores`): 2%, which a road tunnel is built within.
const TUNNEL_FAR_GRADE: f64 = 0.02;
const RAIL_TUNNEL_DEPTH_M: f64 = 3.0;
const RAIL_INTERIOR_HEIGHT_M: f64 = 4.0;
/// A block, as a length: what Arnis adds or takes by one.
const BLOCK_M: f64 = 1.0;

/// The level whose approaches run furthest (`approach_reach`).
pub(super) const DEEPEST: i64 = -(TUNNEL_MAX_LAYERS_BELOW + 1);

/// How far a portal's approaches can run for a road tunnel at `level`, in DB coordinates.
pub(super) fn approach_reach(level: i64, scale: Scale) -> f64 {
    mer_from_u(f64::from(tunnel_drop(level, scale) * TUNNEL_RAMP_RUN) * CELL_U)
}

/// The bores of the stretches in scope, road tunnels and rail tunnels, as lines; a road tunnel's portal
/// on a deck opens at it (`decks`).
pub(super) fn bores(
    src: &Ring,
    stretches: &[Stretch],
    decks: &Decks,
    scale: Scale,
    lines: &mut Vec<(Line, Drawn)>,
) {
    let ceiling = (scale.c(TUNNEL_CEIL_OFFSET_M) - scale.c(BLOCK_M)).clamp(0, 255) as u8;
    for (q, ys, covered) in road_bores(src, stretches, decks, scale, lines) {
        let r = &stretches[q];
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
            lines.push(line_of(r, &ys, stretch, form, clear));
        }
    }
    for q in (0..stretches.len())
        .filter(|&q| stretches[q].piece.form == Form::Tunnel && stretches[q].rail())
    {
        // the floor under the interior, under the ceiling, under the cover (Arnis `generate_rail_tunnel_shell`)
        let depth = scale.c(RAIL_TUNNEL_DEPTH_M + RAIL_INTERIOR_HEIGHT_M + BLOCK_M);
        let ys: Vec<i32> = stretches[q].terrain.iter().map(|t| t - depth).collect();
        lines.push(line_of(
            &stretches[q],
            &ys,
            [0, ys.len() - 1],
            Form::Tunnel,
            scale.c(RAIL_INTERIOR_HEIGHT_M).clamp(0, 255) as u8,
        ));
    }
}

/// Arnis's `is_pedestrian_way_tags`: what a tunnel's approach matches on foot.
fn is_pedestrian(class: &str) -> bool {
    matches!(class, "footway" | "pedestrian" | "steps")
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
fn tunnel_drop(level: i64, scale: Scale) -> i32 {
    scale.c(TUNNEL_COVER_DROP_M)
        + (-(level + 1)).clamp(0, TUNNEL_MAX_LAYERS_BELOW) as i32 * scale.c(TUNNEL_LAYER_DROP_M)
}

fn road_bores(
    src: &Ring,
    stretches: &[Stretch],
    decks: &Decks,
    scale: Scale,
    lines: &mut Vec<(Line, Drawn)>,
) -> Vec<(usize, Vec<i32>, Vec<bool>)> {
    let ramp_ways = &decks.ramp_ways;
    let tunnels: Vec<usize> = (0..stretches.len())
        .filter(|&q| stretches[q].piece.form == Form::Tunnel && !stretches[q].rail())
        .collect();
    // a node two tunnels share, or an end the ring cut, stays at depth
    let mut owner: FnvHashMap<&End, PartId> = FnvHashMap::default();
    let mut shared: FnvHashSet<&End> = FnvHashSet::default();
    for &q in &tunnels {
        for n in &stretches[q].nodes {
            let id = stretches[q].piece.id();
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
    let internal =
        |q: usize, e: usize| stretches[q].clipped[e] || shared.contains(&stretches[q].ends[e]);
    let drop_of = |r: &Stretch| tunnel_drop(r.piece.way.tags.level.unwrap_or(0), scale);

    // ---- a portal on a road deck, at its end or along it, opens at the deck's height there, where Arnis
    // leaves it at the terrain under the deck: the bore ramps down from it, with no approach to sink ----
    let mut deck: FnvHashMap<&End, i32> = FnvHashMap::default();
    for &q in &tunnels {
        for e in (0..2).filter(|&e| !internal(q, e)) {
            let (node, p) = (
                &stretches[q].ends[e],
                stretches[q].at[if e == 0 { 0 } else { stretches[q].at.len() - 1 }],
            );
            let on = decks
                .ys
                .iter()
                .filter(|(b, _)| stretches[*b].nodes.contains(node))
                .filter_map(|(b, ys)| {
                    let near = |k: &usize| {
                        libm::hypot(
                            stretches[*b].at[*k][0] - p[0],
                            stretches[*b].at[*k][1] - p[1],
                        )
                    };
                    // the stretch of the deck that passes it: a cell of it within a cell of the portal
                    (0..ys.len())
                        .min_by(|i, j| near(i).total_cmp(&near(j)))
                        .filter(|k| near(k) <= mer_from_u(CELL_U))
                        .map(|k| ys[k])
                })
                .max();
            if let Some(y) = on {
                deck.insert(node, y);
            }
        }
    }

    // ---- the approaches: ways at grade from a portal, as far as the drop needs and the road goes on ----
    let portal_nodes: FnvHashSet<&End> = tunnels
        .iter()
        .flat_map(|&q| {
            (0..2)
                .filter(move |&e| !internal(q, e))
                .map(move |e| &stretches[q].ends[e])
        })
        .collect();
    let surface = |part: &PartId| {
        src.ways.get(part).filter(|w| {
            !w.row.tags.bridge
                && !w.row.tags.tunnel
                && w.row.subtype == "road"
                && !ramp_ways.contains(part)
        })
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
        let r = &stretches[q];
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
            let eligible = |at: &End,
                            taken: &FnvHashSet<PartId>,
                            claims: &BTreeMap<PartId, Vec<Claim>>|
             -> Vec<PartId> {
                let mut out: Vec<PartId> = src
                    .way_ends
                    .get(at)
                    .into_iter()
                    .flatten()
                    .filter(|g| !taken.contains(*g) && claims.get(*g).is_none_or(|c| c.len() < 2))
                    .filter(|g| {
                        surface(g).is_some_and(|w| {
                            is_pedestrian(w.row.class.as_deref().unwrap_or("")) == walking
                        })
                    })
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
                    if leaves
                        || dist >= budget
                        || chain.len() >= TUNNEL_APPROACH_WAYS
                        || portal_nodes.contains(&at)
                    {
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
            let Some(avail) = branches
                .iter()
                .filter_map(|b| b.last().map(|(_, _, d0, len)| d0 + len - 1))
                .min()
            else {
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
            .map(|c| {
                [
                    (c.anchor - c.drop * c.run).max(0),
                    (c.anchor + c.drop * c.run).min(last),
                ]
            })
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
                                let drop = cs
                                    .iter()
                                    .map(|c| c.drop - (k - c.anchor).abs() / c.run)
                                    .max()
                                    .unwrap_or(0)
                                    .max(0);
                                [p[0], p[1], f64::from(t - drop)]
                            })
                            .collect();
                        let fraction = |k: i32| w.row.span[0] + along(k) / w.row.segment_length;
                        lines.push(graded(
                            w,
                            points,
                            [from == 0, k - 1 == last],
                            [fraction(from), fraction(k - 1)],
                        ));
                    }
                }
                k += 1;
            }
        }
    }

    // ---- each bore (Arnis `generate_highway_tunnel_shell`); the world has no bedrock to stop it ----
    let roof = scale.c(TUNNEL_ROOF_COVER_M);
    let mut out = Vec::new();
    for &q in &tunnels {
        let r = &stretches[q];
        let n = r.cell.len();
        let last = n - 1;
        // the lowest ground over the bore's footprint decides its roof
        let wall = disk(way::footprint((r.width_u / CELL_U) as f32));
        let cover: Vec<i32> = r
            .at
            .iter()
            .zip(&r.terrain)
            .map(|(p, &t)| {
                let around: Vec<[f64; 2]> = wall
                    .iter()
                    .map(|&[dx, dz]| {
                        [
                            p[0] + mer_from_u(dx as f64 * CELL_U),
                            p[1] + mer_from_u(dz as f64 * CELL_U),
                        ]
                    })
                    .filter(|q| src.loaded.contains(&tile_of(*q)))
                    .collect();
                sample_cells(&src.ground, &around).map_or(t, |v| v.into_iter().fold(t, i32::min))
            })
            .collect();
        let (internal_s, internal_e) = (internal(q, 0), internal(q, 1));
        let deck_s = if internal_s {
            None
        } else {
            deck.get(&r.ends[0]).copied()
        };
        let deck_e = if internal_e {
            None
        } else {
            deck.get(&r.ends[1]).copied()
        };
        let (gs, ge) = (
            deck_s.unwrap_or(r.terrain[0]),
            deck_e.unwrap_or(r.terrain[last]),
        );
        let drop_cover = drop_of(r);
        let start_drop = if internal_s {
            0
        } else {
            drops.get(&r.ends[0]).copied().unwrap_or(0)
        };
        let end_drop = if internal_e {
            0
        } else {
            drops.get(&r.ends[1]).copied().unwrap_or(0)
        };
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
            envelope(&room, TUNNEL_FAR_GRADE, TILE_CELLS as usize)
                .into_iter()
                .map(|y| round(y) as i32)
                .collect()
        } else {
            (0..n)
                .map(|i| grade(i).min(cover[i] - drop_cover))
                .collect::<Vec<i32>>()
        };
        // the ground caps the bore, but where a deck's portal holds it over the ground, ramping down from
        // the deck, and in a short bore no higher than the straight run between the portals, so it meets
        // the other one
        let over = |i: usize| {
            [
                deck_s.map(|d| d - i as i32 / TUNNEL_RAMP_RUN),
                deck_e.map(|d| d - (last - i) as i32 / TUNNEL_RAMP_RUN),
            ]
            .into_iter()
            .flatten()
            .max()
            .map_or(r.terrain[i], |held| {
                r.terrain[i].max(if long { held } else { held.min(grade(i)) })
            })
        };
        // as deep as cover asks, ramping down from whatever the portals reached
        let mut ys: Vec<i32> = (0..n)
            .map(|i| {
                let desired = floor[i];
                let ramp_s = if internal_s {
                    i32::MIN
                } else {
                    gs - start_drop - i as i32 / TUNNEL_RAMP_RUN
                };
                let ramp_e = if internal_e {
                    i32::MIN
                } else {
                    ge - end_drop - (last - i) as i32 / TUNNEL_RAMP_RUN
                };
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
