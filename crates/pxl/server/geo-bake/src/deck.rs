// pxl-geo-bake/deck.rs

//! The decks' heights and the tunnels' mouths, solved offline over the whole
//! network and written to `overture_deck`, which the terrain bake reads a page at a time
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
//! - `piece`: a segment's bridge or tunnel ranges, less what is not seen or
//!   not built.
//! - `approach`: the way at the surface each end of a bridge is graded
//!   down to the ground on.
//! - `structure`: the pieces that meet end to end or run side by side, and
//!   the graph of points along them.
//! - `clearance`: what a deck has to clear, and how steeply it may climb.
//! - `profile`: a structure's tops, from its ends and its clearances.
//! - `elevation`: the ground's height at the points, as the pages draw it.
//! - `source`: the geo DB, read and written.
//!
//! It runs in two passes. A deck that crosses a lower one clears that
//! deck's top, which the first pass solves with nothing stacked; the second
//! solves every structure again with those tops, lower decks included, so a
//! ramp over its own interchange's mainline clears it too.

pub mod approach;
pub mod clearance;
pub mod elevation;
pub mod piece;
pub mod profile;
pub mod source;
pub mod structure;

use postgres::Client;

use clearance::{Under, PIER_MIN_RISE, PIER_OFF_WAY, PIER_STEP, STACKED};
use piece::{pieces_of, Piece};
use profile::{solve as solve_profile, Problem, Profile};
use pxl_spec::bake::constant::CELL_PER_M;
use pxl_spec::bake::deck::{Form, BORE_HEIGHT};
use source::DeckOut;
use structure::{group, side_by_side, Graph};

use crate::terrain::pipeline::overture::between::{flag_at, flag_ranges, value_at};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};

/// How far from a piece's end a crossing is its own joint rather than
/// something under it, in metres: where two pieces meet they intersect at
/// that point.
const AT_END: f64 = 1.0;

/// How deep under the drawn surface a bore's floor lies wherever it is not
/// coming up to a portal, in z16 cells: its air, a cell of the ground over
/// it as its roof, and half a cell for the rounding the floor and the
/// ground each take into cells, so the roof is never rounded away.
const COVER: f64 = BORE_HEIGHT as f64 + 1.5;

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
pub fn solve(client: &mut Client, url: &str, log: impl Fn(&str)) -> Result<Solved, postgres::Error> {
    // One solve at a time: its staging tables are shared by name, and two
    // solves would drop each other's.
    client.execute("SELECT pg_advisory_lock(hashtext('pxl-geo-bake deck solve'))", &[])?;
    let rows = source::flagged_rows(client, "is_bridge")?;
    let mut pieces: Vec<Piece> = rows.iter().flat_map(|r| pieces_of(r, Form::Deck)).collect();
    log(&format!("{} bridge segments, {} deck pieces", rows.len(), pieces.len()));
    drop(rows);

    source::stage(client, &pieces)?;
    let mut needs = needs_of(client, url, &pieces)?;
    let ramps = approach::approaches(url, &pieces)?;
    log(&format!("{} approaches graded with their bridges", ramps.len()));
    pieces.extend(ramps);
    needs.resize(pieces.len(), Vec::new());
    let crossings: Vec<source::DeckCrossing> = source::decks_crossing(url)?
        .into_iter()
        .map(|c| source::DeckCrossing {
            upper_at: c.upper_at / pieces[c.upper].scale(),
            lower_at: c.lower_at / pieces[c.lower].scale(),
            ..c
        })
        .collect();
    log(&format!(
        "{} ways and waters under decks, {} decks over decks",
        needs.iter().map(Vec::len).sum::<usize>(),
        crossings.len()
    ));

    let beside = side_by_side(&pieces);
    let structures = group(&pieces, &beside);
    let graphs: Vec<Graph> = structures.iter().map(|m| Graph::build(&pieces, m, &beside)).collect();
    // Which structure each piece is in, and its place among the members.
    let mut place = vec![(0usize, 0usize); pieces.len()];
    for (s, members) in structures.iter().enumerate() {
        for (k, &m) in members.iter().enumerate() {
            place[m] = (s, k);
        }
    }
    let tunnel_rows = source::flagged_rows(client, "is_tunnel")?;
    let tunnels: Vec<Piece> = tunnel_rows.iter().flat_map(|r| pieces_of(r, Form::Bore)).collect();
    drop(tunnel_rows);
    let bored = group(&tunnels, &[]);
    let bore_graphs: Vec<Graph> = bored.iter().map(|m| Graph::build(&tunnels, m, &[])).collect();

    // The ground under every point of both, read once: a page a deck and a
    // tunnel share is one page.
    let points = graphs.iter().map(|g| g.at.len()).sum::<usize>();
    let all: Vec<[f64; 2]> = graphs.iter().chain(&bore_graphs).flat_map(|g| g.at.iter().copied()).collect();
    let mut flat = elevation::surface(client, url, &all)?.into_iter();
    let mut ground_of = |graphs: &[Graph]| -> Vec<Vec<f64>> { graphs.iter().map(|g| flat.by_ref().take(g.at.len()).collect()).collect() };
    let grounds = ground_of(&graphs);
    let bore_grounds = ground_of(&bore_graphs);
    log(&format!("{} structures, {} pairs of decks side by side, {} points", structures.len(), beside.len(), points));

    let carried: Vec<Vec<bool>> = structures.iter().zip(&graphs).map(|(members, g)| carried_of(&pieces, members, g)).collect();
    let mut tops: Vec<Vec<f64>> = grounds.clone();
    let mut slopes = Slopes::default();
    for pass in 0..2 {
        slopes = Slopes::default();
        let mut next = Vec::with_capacity(structures.len());
        for (s, members) in structures.iter().enumerate() {
            let (g, ground) = (&graphs[s], &grounds[s]);
            let mut need = ground.clone();
            for (k, &m) in members.iter().enumerate() {
                for n in &needs[m] {
                    for node in nodes_within(g, k, n.from, n.to) {
                        need[node] = need[node].max(ground[node] + n.under.headroom(n.to - n.from));
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
            let profile = solve_profile(&Problem {
                adj: &g.adj,
                need: &need,
                ends: &ends,
                ground,
                carried: &carried[s],
            });
            slopes.count(&profile);
            next.push(profile.top);
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
            form: piece.form,
            points: Vec::with_capacity(g.along[k].len()),
            piers: Vec::with_capacity(g.along[k].len()),
        };
        for &(node, d) in on_structure(piece, &g.along[k], |n| top[n] > ground[n]) {
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
    log(&slopes.say("deck"));
    let (bores, bore_slopes) = bores_of(url, &tunnels, &bored, &bore_graphs, &bore_grounds)?;
    log(&format!("{} tunnel pieces, {} bored through from a portal", tunnels.len(), bores.len()));
    log(&bore_slopes.say("bore"));
    out.extend(bores);
    source::write(client, &out)?;
    Ok(Solved {
        pieces: pieces.len(),
        structures: structures.len(),
        points,
        piers,
    })
}

/// The bores of the tunnels that come out to the ground, cut their whole
/// length so a first-person view can go through. A bore's floor is a deck's
/// profile turned over (profile.rs), one rule for a road tunnel and a
/// subway alike: it meets each portal at the portal's ground, runs straight
/// between them, and is lowered under the drawn surface wherever that line
/// would leave less than `COVER` over it — falling off at the steepest a
/// bore descends, so it comes down from a portal on a ramp and then follows
/// the ground at that depth, its roof the ground. Only the ramps at the
/// portals are open to the sky. A tunnel with one portal has no line beyond
/// that portal's height, and lies under the ground the rest of its length.
/// A portal is an end where the way carries on at the surface — a way
/// touching it there that is no tunnel and stands at level 0 or over — and
/// not merely an end: a subway's tunnel ends at its stations' platforms,
/// which are under the ground too, and a tunnel with no portal is cut
/// nowhere.
fn bores_of<'a>(url: &str, tunnels: &'a [Piece], structures: &[Vec<usize>], graphs: &[Graph], grounds: &[Vec<f64>]) -> Result<(Vec<DeckOut<'a>>, Slopes), postgres::Error> {
    let ends: Vec<([f64; 2], &str)> = structures
        .iter()
        .zip(graphs)
        .flat_map(|(members, g)| g.free_ends.iter().zip(&g.free_end_of).map(|(&e, &k)| (g.at[e as usize], tunnels[members[k]].gers_id.as_str())))
        .collect();
    let mut portal = vec![false; ends.len()];
    for t in source::touching(url, &ends)? {
        let at_surface = |at: f64| (0.0..=1.0).contains(&at) && !flag_at(t.flags.as_deref(), UNSEEN_FLAGS, at) && value_at(t.levels.as_deref(), at).unwrap_or(0.0) >= 0.0;
        if at_surface(t.own_at - t.step) || at_surface(t.own_at + t.step) {
            portal[t.point] = true;
        }
    }
    let mut end_at = 0;
    let mut out = Vec::new();
    let mut slopes = Slopes::default();
    for ((members, g), ground) in structures.iter().zip(graphs).zip(grounds) {
        let ends: Vec<(u32, f64)> = g.free_ends.iter().enumerate().filter(|&(j, _)| portal[end_at + j]).map(|(_, &e)| (e, ground[e as usize])).collect();
        end_at += g.free_ends.len();
        if ends.is_empty() {
            continue;
        }
        // Turned over, the floor's depth under the ground is a deck's height
        // over it: the line between the portals, and a cover it must keep
        // under the ground, which lowers it about the point that asks.
        let below = |x: &[f64]| x.iter().map(|v| -v).collect::<Vec<f64>>();
        let need: Vec<f64> = g.at.iter().zip(ground).map(|(at, &h)| COVER / CELL_PER_M(at[1]) - h).collect();
        let ends: Vec<(u32, f64)> = ends.iter().map(|&(e, h)| (e, -h)).collect();
        let carried = vec![true; g.at.len()];
        let profile = solve_profile(&Problem {
            adj: &g.adj,
            need: &need,
            ends: &ends,
            ground: &below(ground),
            carried: &carried,
        });
        slopes.count(&profile);
        let floor = below(&profile.top);
        for (k, &m) in members.iter().enumerate() {
            let points: Vec<[f64; 3]> = g.along[k].iter().map(|&(node, _)| [g.at[node as usize][0], g.at[node as usize][1], floor[node as usize]]).collect();
            if points.len() >= 2 {
                out.push(DeckOut {
                    piece: &tunnels[m],
                    form: Form::Bore,
                    piers: vec![0; points.len()],
                    points,
                });
            }
        }
    }
    Ok((out, slopes))
}

/// The steps of the solved structures that are steeper than `MAX_SLOPE`:
/// each one lies on a stretch between two ends whose own ground is that
/// steep apart (profile.rs `Profile::forced`), which no deck or bore could
/// meet, and the count says how much of the data asks for it.
#[derive(Default)]
struct Slopes {
    forced: usize,
}

impl Slopes {
    fn count(&mut self, profile: &Profile) {
        self.forced += profile.forced.iter().filter(|&&f| f).count();
    }

    fn say(&self, what: &str) -> String {
        format!("{what}: {} points on stretches whose ends are steeper apart than a {what} may climb", self.forced)
    }
}

/// Whether each point of a structure is carried: on a member that is the
/// bridge's own rather than an approach to it. A joint is
/// both, and carried.
fn carried_of(pieces: &[Piece], members: &[usize], g: &Graph) -> Vec<bool> {
    let mut carried = vec![false; g.at.len()];
    for (k, &m) in members.iter().enumerate() {
        if !pieces[m].approach {
            for &(n, _) in &g.along[k] {
                carried[n as usize] = true;
            }
        }
    }
    carried
}

/// A member's points as far as it is off the ground: the whole of a
/// bridge, and an approach from where it joins as far as the point it
/// comes to the ground, past which it is the way on the ground it was.
fn on_structure<'a>(piece: &Piece, along: &'a [(u32, f64)], off_ground: impl Fn(usize) -> bool) -> &'a [(u32, f64)] {
    if !piece.approach {
        return along;
    }
    let down = along.iter().position(|&(n, _)| !off_ground(n as usize)).map_or(along.len(), |i| i + 1);
    &along[..down]
}

/// What passes under each piece: the ways that cross it where they are on
/// the ground — not where they are a tunnel, nor a deck, which is cleared as
/// a deck — and the water it spans.
fn needs_of(client: &mut Client, url: &str, pieces: &[Piece]) -> Result<Vec<Vec<Need>>, postgres::Error> {
    let mut needs: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
    // Where along a piece, in physical metres, from PostGIS's Mercator ones.
    let physical = |m: usize, at: f64| at / pieces[m].scale();
    let length = |m: usize| {
        let p = &pieces[m].points;
        physical(m, p.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>())
    };
    let inside = |at: f64, m: usize| at > AT_END && at < length(m) - AT_END;
    for mut c in source::ways_crossing(url)? {
        c.at = physical(c.piece, c.at);
        let off_ground = flag_ranges(c.flags.as_deref(), UNSEEN_FLAGS)
            .iter()
            .chain(&flag_ranges(c.flags.as_deref(), DECK_FLAGS))
            .any(|&[a, b]| (a..=b).contains(&c.own_at));
        if off_ground || !inside(c.at, c.piece) {
            continue;
        }
        let under = Under::of_way(&c.subtype, c.class.as_deref());
        needs[c.piece].push(Need { from: c.at, to: c.at, under });
    }
    let mut water: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
    for w in source::water_under(client, url)? {
        if let Some(under) = Under::of_water(&w.subtype, w.class.as_deref()) {
            water[w.piece].push(Need {
                from: physical(w.piece, w.from),
                to: physical(w.piece, w.to),
                under,
            });
        }
    }
    // One stretch of water is many pieces once its polygon is subdivided, and
    // how much of it a deck spans is the stretch's: the pieces of one kind
    // that touch are joined first.
    for (m, mut spans) in water.into_iter().enumerate() {
        spans.sort_by(|a, b| a.from.total_cmp(&b.from));
        for n in spans {
            match needs[m].last_mut() {
                Some(last) if last.under == n.under && n.from <= last.to + AT_END => last.to = last.to.max(n.to),
                _ => needs[m].push(n),
            }
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
