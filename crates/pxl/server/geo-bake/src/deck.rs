// pxl-geo-bake/deck.rs

//! The decks' heights, solved offline over the whole network and written to
//! `overture_deck`, which the terrain bake reads a page at a time
//! (terrain/pipeline/deck.rs).
//!
//! A deck's height depends on where its structure comes down to the ground,
//! which may be kilometres and many pages away, and on what passes under
//! it, which only the vectors say — so it is solved here, once, and never by
//! a page or by the client (issue/20261001-ways-off-the-ground.md). The
//! solve is Arnis's, reinterpreted: structure by structure, not the whole
//! network as one system, which is the approach OSM2World found too fragile
//! to turn on.
//!
//! - `piece`: a segment's bridge ranges, less what is not seen.
//! - `structure`: the pieces that meet end to end, and the graph of points
//!   along them.
//! - `clearance`: what a deck has to clear, and how steeply it may climb.
//! - `profile`: a structure's tops, from its ends and its clearances.
//! - `elevation`: the ground's height at the points, a raster tile at a time.
//! - `source`: the geo DB, read and written.
//!
//! It runs in two passes. A deck that crosses a lower one clears that
//! deck's top, which the first pass solves with nothing stacked; the second
//! solves every structure again with those tops, lower decks included, so a
//! ramp over its own interchange's mainline clears it too.

pub mod clearance;
pub mod elevation;
pub mod piece;
pub mod profile;
pub mod source;
pub mod structure;

use postgres::Client;

use clearance::{Under, PIER_MIN_RISE, PIER_OFF_WAY, PIER_STEP, STACKED};
use piece::{pieces_of, Piece};
use profile::{solve as solve_profile, Problem};
use source::DeckOut;
use structure::{group, Graph};

use crate::terrain::pipeline::overture::between::flag_ranges;
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};

/// How far from a piece's end a crossing is its own joint rather than
/// something under it, in metres: where two pieces meet they intersect at
/// that point.
const AT_END: f64 = 1.0;

/// What one solve made.
pub struct Solved {
    pub pieces: usize,
    pub structures: usize,
    pub points: usize,
    pub piers: usize,
}

/// What passes under a piece, from and to metres along it.
#[derive(Clone, Copy)]
struct Need {
    from: f64,
    to: f64,
    under: Under,
}

/// Solves every deck and writes `overture_deck`, whole.
pub fn solve(client: &mut Client, log: impl Fn(&str)) -> Result<Solved, postgres::Error> {
    let rows = source::bridge_rows(client)?;
    let pieces: Vec<Piece> = rows.iter().flat_map(pieces_of).collect();
    log(&format!("{} bridge segments, {} deck pieces", rows.len(), pieces.len()));
    drop(rows);

    source::stage(client, &pieces)?;
    let needs = needs_of(client, &pieces)?;
    let crossings = source::decks_crossing(client)?;
    log(&format!("{} ways and waters under decks, {} decks over decks", needs.iter().map(Vec::len).sum::<usize>(), crossings.len()));

    let structures = group(&pieces);
    let graphs: Vec<Graph> = structures.iter().map(|m| Graph::build(&pieces, m)).collect();
    // Which structure each piece is in, and its place among the members.
    let mut place = vec![(0usize, 0usize); pieces.len()];
    for (s, members) in structures.iter().enumerate() {
        for (k, &m) in members.iter().enumerate() {
            place[m] = (s, k);
        }
    }
    let all: Vec<[f64; 2]> = graphs.iter().flat_map(|g| g.at.iter().copied()).collect();
    let flat = elevation::ground(client, &all)?;
    let mut grounds: Vec<Vec<f64>> = Vec::with_capacity(graphs.len());
    let mut at = 0;
    for g in &graphs {
        grounds.push(flat[at..at + g.at.len()].to_vec());
        at += g.at.len();
    }
    log(&format!("{} structures, {} points", structures.len(), all.len()));

    let mut tops: Vec<Vec<f64>> = grounds.clone();
    for pass in 0..2 {
        let mut next = Vec::with_capacity(structures.len());
        for (s, members) in structures.iter().enumerate() {
            let (g, ground) = (&graphs[s], &grounds[s]);
            let mut need = ground.clone();
            for (k, &m) in members.iter().enumerate() {
                for n in &needs[m] {
                    for node in nodes_within(g, k, n.from, n.to) {
                        need[node] = need[node].max(ground[node] + n.under.headroom());
                    }
                }
            }
            if pass == 1 {
                for c in crossings.iter().filter(|c| place[c.upper].0 == s) {
                    let (ls, lk) = place[c.lower];
                    let lower_top = top_along(&graphs[ls], &tops[ls], lk, c.lower_at);
                    if let Some(node) = g.nearest(place[c.upper].1, c.upper_at) {
                        need[node as usize] = need[node as usize].max(lower_top + STACKED);
                    }
                }
            }
            let ends: Vec<(u32, f64)> = g.free_ends.iter().map(|&e| (e, ground[e as usize])).collect();
            next.push(solve_profile(&Problem { adj: &g.adj, need: &need, ends: &ends }));
        }
        tops = next;
    }

    let mut out = Vec::with_capacity(pieces.len());
    let mut piers = 0;
    for (m, piece) in pieces.iter().enumerate() {
        let (s, k) = place[m];
        let (g, ground, top) = (&graphs[s], &grounds[s], &tops[s]);
        let keep_off: Vec<&Need> = needs[m].iter().filter(|n| n.under.keeps_piers_off()).collect();
        let mut since = 0.0;
        let mut last_d = 0.0;
        let mut deck = DeckOut {
            piece,
            points: Vec::with_capacity(g.along[k].len()),
            piers: Vec::with_capacity(g.along[k].len()),
        };
        for &(node, d) in &g.along[k] {
            let n = node as usize;
            since += d - last_d;
            last_d = d;
            let clear = keep_off.iter().all(|w| d < w.from - PIER_OFF_WAY || d > w.to + PIER_OFF_WAY);
            let pier = since >= PIER_STEP && top[n] - ground[n] >= PIER_MIN_RISE && clear;
            if pier {
                since = 0.0;
                piers += 1;
            }
            deck.points.push([g.at[n][0], g.at[n][1], top[n]]);
            deck.piers.push(u8::from(pier));
        }
        if deck.points.len() >= 2 {
            out.push(deck);
        }
    }
    source::write(client, &out)?;
    Ok(Solved {
        pieces: pieces.len(),
        structures: structures.len(),
        points: all.len(),
        piers,
    })
}

/// What passes under each piece: the ways that cross it where they are on
/// the ground — not where they are a tunnel, nor a deck, which is cleared as
/// a deck — and the water it spans.
fn needs_of(client: &mut Client, pieces: &[Piece]) -> Result<Vec<Vec<Need>>, postgres::Error> {
    let mut needs: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
    let length = |m: usize| {
        let p = &pieces[m].points;
        p.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>()
    };
    let inside = |at: f64, m: usize| at > AT_END && at < length(m) - AT_END;
    for c in source::ways_crossing(client)? {
        let off_ground = flag_ranges(c.flags.as_deref(), UNSEEN_FLAGS).iter().chain(&flag_ranges(c.flags.as_deref(), DECK_FLAGS)).any(|&[a, b]| (a..=b).contains(&c.own_at));
        if off_ground || !inside(c.at, c.piece) {
            continue;
        }
        let under = Under::of_way(&c.subtype, c.class.as_deref());
        needs[c.piece].push(Need { from: c.at, to: c.at, under });
    }
    for w in source::water_under(client)? {
        if let Some(under) = Under::of_water(&w.subtype, w.class.as_deref()) {
            needs[w.piece].push(Need { from: w.from, to: w.to, under });
        }
    }
    Ok(needs)
}

/// The points of member `k` from `from` to `to` metres along it, or the
/// nearest one where the two are one point.
fn nodes_within(g: &Graph, k: usize, from: f64, to: f64) -> Vec<usize> {
    let within: Vec<usize> = g.along[k].iter().filter(|&&(_, d)| d >= from && d <= to).map(|&(n, _)| n as usize).collect();
    if within.is_empty() {
        g.nearest(k, (from + to) / 2.0).map(|n| vec![n as usize]).unwrap_or_default()
    } else {
        within
    }
}

/// A member's top `d` metres along it, between its points.
fn top_along(g: &Graph, tops: &[f64], k: usize, d: f64) -> f64 {
    let nodes = &g.along[k];
    let i = nodes.partition_point(|&(_, nd)| nd < d).clamp(1, nodes.len().max(2) - 1);
    let ((a, da), (b, db)) = (nodes[i - 1], nodes[i]);
    let t = if db > da { ((d - da) / (db - da)).clamp(0.0, 1.0) } else { 0.0 };
    tops[a as usize] + (tops[b as usize] - tops[a as usize]) * t
}
