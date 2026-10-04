// pxl-geo-bake/deck.rs

//! Deck heights and tunnel bores, solved offline over the whole network into
//! `overture_deck`. A deck's height depends on ground kilometres away, so no
//! page or client can solve it. Structure by structure, as Arnis does.
//!
//! Two passes: the first solves with nothing stacked, the second again with
//! the lower decks' tops, so a ramp over its own interchange clears it.

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

/// Metres from a piece's end within which a crossing is its own joint.
const AT_END: f64 = 1.0;

pub struct Solved {
    pub pieces: usize,
    pub structures: usize,
    pub points: usize,
}

#[derive(Clone, Copy)]
struct Need {
    from: f64,
    to: f64,
    under: Under,
}

pub fn solve(client: &mut Client, url: &str, log: impl Fn(&str)) -> Result<Solved, postgres::Error> {
    // One solve at a time: two would drop each other's staging tables.
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

/// A structure's end with nothing of its form past it: its surface, and how
/// far that stands off the ground.
struct Free {
    piece: Piece,
    last: bool,
    top: f64,
    sep: f64,
}

/// A range of a way, as fractions of it, that a line draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub gers_id: String,
    pub range: [f64; 2],
}

/// What a solve reads through: the DB, each page's rows, and where solved
/// structures are kept.
pub struct Reader<'a, S: Store> {
    pub client: &'a mut Client,
    pub repo: &'a TerrainRepository,
    pub store: &'a mut S,
    pub memo: &'a Path,
}

/// A page's lines: every structure reaching it or the eight around it, and
/// the ways their ends go on along there.
pub fn solve_page(r: &mut Reader<impl Store>, page: IVec) -> Result<Solved, String> {
    let ring = source::read_pages(r.client, r.repo, &source::ring(page), r.store)?;
    let mut out = Solved::default();
    let mut free: Vec<Free> = Vec::new();
    for form in [Form::Deck, Form::Bore] {
        let seeds: Vec<Piece> = ring.rows.iter().flat_map(|row| pieces_of(row, form)).filter(|p| p.touches(ring.bounds)).collect();
        for members in whole(r.client, seeds, form)? {
            let profile = solve_structure(r, &members, true)?;
            for (p, run) in members.iter().zip(&profile.runs) {
                let (first, last) = (run.points[0], run.points[run.points.len() - 1]);
                let (a, b) = (p.on_way(first[0]), p.on_way(last[0]));
                out.drawn.push(Drawn {
                    gers_id: p.gers_id.clone(),
                    range: [a.min(b), a.max(b)],
                });
                out.lines.push(Line {
                    subtype: p.subtype.clone(),
                    class: p.class.clone(),
                    width_m: p.width_m,
                    form,
                    points: run.points.iter().map(|q| [q[1], q[2], q[3]]).collect(),
                });
            }
            for (i, last) in free_ends(&members) {
                let run = &profile.runs[i];
                let top = run.points[if last { run.points.len() - 1 } else { 0 }][3];
                free.push(Free {
                    piece: members[i].clone(),
                    last,
                    top,
                    sep: top - run.ground[usize::from(last)],
                });
            }
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

    // each free end's ground stretches, sampled where the ring's ground is held
    let asked: Vec<approach::Free> = free.iter().map(|f| approach_of(&f.piece, f.last)).collect();
    let inside = |p: [f64; 2]| (0..2).all(|k| p[k] >= ring.bounds[0][k] && p[k] < ring.bounds[1][k]);
    // where a road meets the ground, once a point, so two ending on one share it
    let mut meeting: HashMap<(usize, u64), [f64; 3]> = HashMap::new();
    for b in &branches(&ring.ways, &asked, inside) {
        let ground = elevation::sample(&ring.ground, &b.at.iter().map(|&(_, p)| p).collect::<Vec<_>>())?;
        let end = &free[b.end];
        let to = approach::meets(b, end.sep, b.across.map(|o| free[o].sep));
        let mut points = approach::profile(b, end.top, &ground, to);
        if points.is_empty() {
            continue;
        }
        // past the ring the road is no page's around this one, and its ground is not held
        let way = &ring.ways[b.way];
        let at = way.line.at(to * way.line.length());
        if inside(at) {
            let meet = match meeting.get(&(b.way, to.to_bits())) {
                Some(&p) => p,
                None => *meeting.entry((b.way, to.to_bits())).or_insert([at[0], at[1], elevation::sample(&ring.ground, &[at])?[0]]),
            };
            points.push(meet);
        }
        if points.len() < 2 {
            continue;
        }
        let row = &way.row;
        out.drawn.push(Drawn {
            gers_id: row.gers_id.clone(),
            range: [b.from.min(to), b.from.max(to)],
        });
        out.lines.push(Line {
            subtype: row.subtype.clone(),
            class: row.class.clone(),
            width_m: width_m(&row.subtype, row.class.as_deref(), WayShape::at(row.columns(), (b.from + to) / 2.0)),
            form: Form::Grade,
            points,
        });
    }
    Ok(out)
}

/// A free end as the approach asks it.
fn approach_of(p: &Piece, last: bool) -> approach::Free<'_> {
    let end = p.end(last);
    let at = match end {
        End::Vertex(..) => f64::from(u8::from(last)),
        End::Cut(_, bits) => f64::from_bits(*bits),
    };
    approach::Free {
        end,
        gers_id: &p.gers_id,
        at,
        onward: last,
    }
}

/// The whole structures `seeds` are in: each free end at a segment's end asked
/// of the DB for the pieces of `form` that share it, until none is new; each
/// structure's pieces in one order, so it is one key whoever asks.
fn whole(client: &mut Client, mut pieces: Vec<Piece>, form: Form) -> Result<Vec<Vec<Piece>>, String> {
    let id = |p: &Piece| (p.gers_id.clone(), p.start.to_bits());
    let mut have: HashSet<(String, u64)> = pieces.iter().map(id).collect();
    let mut asked: HashSet<End> = HashSet::new();
    loop {
        let open: Vec<End> = free_ends(&pieces)
            .into_iter()
            .map(|(i, last)| pieces[i].end(last).clone())
            .filter(|e| matches!(e, End::Vertex(..)) && !asked.contains(e))
            .collect();
        if open.is_empty() {
            break;
        }
        for end in open {
            let End::Vertex(x, y) = end else { unreachable!() };
            for row in source::flagged_at(client, [f64::from_bits(x), f64::from_bits(y)])? {
                for p in pieces_of(&row, form).into_iter().filter(|p| p.ends.contains(&end)) {
                    if have.insert(id(&p)) {
                        pieces.push(p);
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

/// A member `far` must stand `gap` over a surface at `near_top`, under where `gap` is negative.
struct Crossing {
    far: usize,
    far_at: f64,
    near_top: f64,
    gap: f64,
}

/// Where the structure's members cross a piece of its form that `relate`
/// orders under it (a deck) or over it (a bore), with that piece's surface
/// there as its own structure solves it free of crossings.
fn crossings(r: &mut Reader<impl Store>, members: &[Piece], lines: &[Polyline], rows: &[Segment]) -> Result<Vec<Crossing>, String> {
    let form = members[0].form;
    let own: HashSet<(&str, u64)> = members.iter().map(Piece::id).collect();
    let others: Vec<Piece> = rows.iter().flat_map(|row| pieces_of(row, form)).filter(|p| !own.contains(&p.id())).collect();
    let pieces: Vec<&Piece> = members.iter().chain(&others).collect();
    let all_lines: Vec<Polyline> = lines
        .iter()
        .map(|l| Polyline::of(l.points.iter().copied()))
        .chain(others.iter().map(|p| Polyline::of(p.points.iter().map(|q| [q[0], q[1]]))))
        .collect();
    let mut out = Vec::new();
    // each structure crossed, as solved free of crossings
    let mut solved: Vec<(Vec<Piece>, Profile)> = Vec::new();
    for c in relate::decks_crossing(&pieces, &all_lines) {
        // a deck over what it crosses, a bore under it
        let (far, far_at, near, near_at) = match form {
            Form::Deck => (c.upper, c.upper_at, c.lower, c.lower_at),
            _ => (c.lower, c.lower_at, c.upper, c.upper_at),
        };
        if far >= members.len() {
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

/// Tunnel ends where the way carries on at the surface at level 0 or over;
/// a subway's tunnel ends at underground platforms, which are not portals.
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

/// A bore's floor is a deck's profile turned over: it meets the ground at its
/// portals and runs under it between. A tunnel with no portal is cut nowhere.
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

/// Steps steeper than `MAX_SLOPE`, forced by their ends' own ground.
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

/// Whether each point is the structure's own rather than an approach; a joint
/// is both, and carried.
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

fn on_structure<'a>(piece: &Piece, along: &'a [(u32, f64)], off_ground: impl Fn(usize) -> bool) -> &'a [(u32, f64)] {
    if !piece.approach {
        return along;
    }
    let down = along.iter().position(|&(n, _)| !off_ground(n as usize)).map_or(along.len(), |i| i + 1);
    &along[..down]
}

/// What passes under each piece: ways on the ground there (a deck below is
/// cleared as a deck), and water.
fn needs_of(client: &mut Client, url: &str, pieces: &[Piece]) -> Result<Vec<Vec<Need>>, postgres::Error> {
    let mut needs: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
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
    // A water body is many pieces once subdivided; join the touching ones so the
    // span is the whole stretch's.
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

fn nodes_within(g: &Graph, k: usize, from: f64, to: f64) -> Vec<usize> {
    let within: Vec<usize> = g.along[k].iter().filter(|&&(_, d)| d >= from && d <= to).map(|&(n, _)| n as usize).collect();
    if within.is_empty() {
        g.nearest(k, (from + to) / 2.0).map(|n| vec![n as usize]).unwrap_or_default()
    } else {
        within
    }
}

fn top_along(g: &Graph, tops: &[f64], k: usize, d: f64) -> f64 {
    let nodes = &g.along[k];
    let i = nodes.partition_point(|&(_, nd)| nd < d).clamp(1, nodes.len().max(2) - 1);
    let ((a, da), (b, db)) = (nodes[i - 1], nodes[i]);
    let t = if db > da { ((d - da) / (db - da)).clamp(0.0, 1.0) } else { 0.0 };
    tops[a as usize] + (tops[b as usize] - tops[a as usize]) * t
}
