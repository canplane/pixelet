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

use std::collections::HashSet;

use postgres::Client;

use clearance::{Under, STACKED};
use piece::{pieces_of, Piece};
use profile::{solve as solve_profile, Problem, Profile};
use pxl_spec::bake::deck::Form;
use source::DeckOut;
use structure::{end_of, group, key_of, side_by_side, Graph};

use crate::terrain::pipeline::overture::between::{flag_at, flag_ranges, value_at};
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
    let ramps = approach::approaches(url, &pieces, &approach::free_ends(&pieces))?;
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
    let portals = portals_of(url, &tunnels)?;
    log(&format!("{} portals", portals.len()));
    let portal_at: HashSet<(i64, i64)> = portals.iter().map(|&(i, last)| key_of([end_of(&tunnels[i], last).0[0], end_of(&tunnels[i], last).0[1], 0.0])).collect();
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
    for (m, piece) in pieces.iter().enumerate() {
        let (s, k) = place[m];
        let (g, ground, top) = (&graphs[s], &grounds[s], &tops[s]);
        let points: Vec<[f64; 3]> = on_structure(piece, &g.along[k], |n| top[n] > ground[n])
            .iter()
            .map(|&(node, _)| [g.at[node as usize][0], g.at[node as usize][1], top[node as usize]])
            .collect();
        if points.len() >= 2 {
            out.push(DeckOut {
                piece,
                form: piece.form,
                points,
                portal_m: Vec::new(),
            });
        }
    }
    log(&slopes.say("deck"));
    let (bores, bore_slopes) = bores_of(&tunnels, &portal_at, &bored, &bore_graphs, &bore_grounds);
    log(&format!("{} tunnel pieces, {} bored through from a portal", tunnels.len(), bores.len()));
    log(&bore_slopes.say("bore"));
    out.extend(bores);
    source::write(client, &out)?;
    Ok(Solved {
        pieces: pieces.len(),
        structures: structures.len(),
        points,
    })
}

/// How far each point of a graph is from the nearest of `sources` along
/// it, in its metres: Dijkstra's, from all of them at once.
fn nearest_along(adj: &[Vec<(u32, f64)>], sources: impl Iterator<Item = u32>) -> Vec<f64> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut dist = vec![f64::INFINITY; adj.len()];
    let mut heap = BinaryHeap::new();
    for s in sources {
        dist[s as usize] = 0.0;
        heap.push(Reverse((0u64, s)));
    }
    while let Some(Reverse((d, n))) = heap.pop() {
        let d = f64::from_bits(d);
        if d > dist[n as usize] {
            continue;
        }
        for &(m, len) in &adj[n as usize] {
            let next = d + len;
            if next < dist[m as usize] {
                dist[m as usize] = next;
                heap.push(Reverse((next.to_bits(), m)));
            }
        }
    }
    dist
}

/// The ends of tunnels that are portals, as (piece, its last end): an end
/// where the way carries on at the surface — a way touching it there that is
/// no tunnel and stands at level 0 or over — and not merely an end: a
/// subway's tunnel ends at its stations' platforms, which are under the
/// ground too.
fn portals_of(url: &str, tunnels: &[Piece]) -> Result<Vec<(usize, bool)>, postgres::Error> {
    let ends = approach::free_ends(tunnels);
    let asked: Vec<([f64; 2], &str)> = ends.iter().map(|&(i, last)| (end_of(&tunnels[i], last).0, tunnels[i].gers_id.as_str())).collect();
    let mut portal = vec![false; ends.len()];
    for t in source::touching(url, &asked)? {
        let at_surface = |at: f64| (0.0..=1.0).contains(&at) && !flag_at(t.flags.as_deref(), UNSEEN_FLAGS, at) && value_at(t.levels.as_deref(), at).unwrap_or(0.0) >= 0.0;
        if at_surface(t.own_at - t.step) || at_surface(t.own_at + t.step) {
            portal[t.point] = true;
        }
    }
    Ok(ends.into_iter().zip(portal).filter(|&(_, p)| p).map(|(e, _)| e).collect())
}

/// The bores of the tunnels that come out to the ground, cut their whole
/// length so a first-person view can go through. A bore's floor is a deck's
/// profile turned over (profile.rs), one rule for a road tunnel and a
/// subway alike: it meets the ground at its portals, where OSM begins the
/// tunnel, and between them runs under the ground, straight where that
/// already holds and no steeper than a way climbs anywhere. How deep under
/// the ground a bore is drawn, and how it opens to its portal, is the
/// client's; the line is the way's floor. One that meets the ground nowhere
/// — a subway between stations — is cut nowhere.
fn bores_of<'a>(tunnels: &'a [Piece], portal_at: &HashSet<(i64, i64)>, structures: &[Vec<usize>], graphs: &[Graph], grounds: &[Vec<f64>]) -> (Vec<DeckOut<'a>>, Slopes) {
    let mut out = Vec::new();
    let mut slopes = Slopes::default();
    for ((members, g), ground) in structures.iter().zip(graphs).zip(grounds) {
        let ends: Vec<(u32, f64)> = g
            .free_ends
            .iter()
            .zip(&g.free_end_of)
            .filter(|&(&e, _)| portal_at.contains(&key_of([g.at[e as usize][0], g.at[e as usize][1], 0.0])))
            .map(|(&e, _)| (e, -ground[e as usize]))
            .collect();
        if ends.is_empty() {
            continue;
        }
        // Turned over, a floor's depth under the ground is a deck's height
        // over it, and it is no higher than the ground.
        let carried = carried_of(tunnels, members, g);
        let need: Vec<f64> = ground.iter().map(|v| -v).collect();
        let below = |x: &[f64]| x.iter().map(|v| -v).collect::<Vec<f64>>();
        let profile = solve_profile(&Problem {
            adj: &g.adj,
            need: &need,
            ends: &ends,
            ground: &below(ground),
            carried: &carried,
        });
        slopes.count(&profile);
        let floor = below(&profile.top);
        let from_portal = nearest_along(&g.adj, ends.iter().map(|&(e, _)| e));
        for (k, &m) in members.iter().enumerate() {
            let piece = &tunnels[m];
            let nodes = on_structure(piece, &g.along[k], |n| floor[n] < ground[n]);
            if nodes.len() >= 2 {
                out.push(DeckOut {
                    piece,
                    form: piece.form,
                    points: nodes.iter().map(|&(n, _)| [g.at[n as usize][0], g.at[n as usize][1], floor[n as usize]]).collect(),
                    portal_m: nodes.iter().map(|&(n, _)| from_portal[n as usize] as f32).collect(),
                });
            }
        }
    }
    (out, slopes)
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
/// bridge's or the tunnel's own rather than an approach to it. A joint is
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
/// bridge or a tunnel, and an approach from where it joins as far as the point it
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
