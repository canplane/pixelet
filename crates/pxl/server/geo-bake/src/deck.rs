// pxl-geo-bake/deck.rs

//! Decks and bores, one solve in `q = sign · z` over a page and the eight
//! around it, the page's lines what comes of it.

pub mod approach;
pub mod elevation;
pub mod param;
pub mod piece;
pub mod profile;
pub mod relate;
pub mod source;
pub mod structure;

use std::collections::HashMap;

use postgres::Client;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::deck::{clearance_m, headroom_m, Form, BORE_COVER_M, DECK_THICKNESS_M};
use pxl_spec::bake::format::DeckRow;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m, G3857};
use pxl_spec::space::pyramid::PAGE_CELLS;
use xpute_core::math::scalar::round;

use crate::terrain::repository::DRAWN_REACH_U;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::terrain::pipeline::overture::between::{cell_of, flag_at, flag_ranges, value_at};
use crate::terrain::pipeline::overture::policy::{segment_type, DECK_FLAGS, UNSEEN_FLAGS};
use param::{AT_END_M, RIVER_M, RIVER_MOST_M, RIVER_PER_M, STREAM_M};
use piece::{pieces_of, End, Piece};
use profile::{solve as solve_profile, Problem};
use relate::{Polyline, SIDE_STEP};
use source::Input;
use structure::{end_of, group, side_by_side, Graph};

/// What passes under a deck.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Under {
    Way(u8), // a way of that transport type
    River,
    Stream,
}

impl Under {
    fn headroom(self, span_m: f64) -> f64 {
        match self {
            Under::Way(kind) => clearance_m(kind),
            Under::River => (RIVER_M + RIVER_PER_M * span_m).min(RIVER_MOST_M),
            Under::Stream => STREAM_M,
        }
    }

    /// None for the sea and a name-only feature.
    fn of_water(subtype: &str, class: Option<&str>) -> Option<Under> {
        match (subtype, class.unwrap_or("")) {
            ("ocean", _) | ("physical", _) => None,
            ("stream", _) | (_, "ditch" | "drain" | "stream") => Some(Under::Stream),
            _ => Some(Under::River),
        }
    }
}

#[derive(Clone, Copy)]
struct Need {
    from: f64,
    to: f64,
    under: Under,
}

/// What the solve decides: each way's ranges off the ground, the one fact the
/// ground's paint and the lines both read, and the lines drawn there.
#[derive(Default)]
pub struct Solved {
    pub carried: Vec<Carried>,
    pub rows: Vec<DeckRow>,
}

/// A range of a way, as fractions of it, carried over the ground or under it;
/// the rest of the way is on the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct Carried {
    pub gers_id: String,
    pub range: [f64; 2],
    pub form: Form,
}

/// A page's lines, from what lies in it and the eight pages around it.
pub fn solve_page(client: &mut Client, page: IVec, store: &mut impl source::Store) -> Result<Solved, String> {
    let input = source::read_ring(client, page, store)?;
    let Some((_, grids)) = &input.window else { unreachable!("a ring is read bounded") };
    let (px, py) = (ivec::X(page), ivec::Y(page));
    let within = |p| {
        let at = elevation::page_of(p);
        ivec::gather(ivec::X(at).clamp(px - 1, px + 1), ivec::Y(at).clamp(py - 1, py + 1), 0)
    };
    calculate(&input, |at| Ok::<_, String>(elevation::sample(grids, at, within)), |_| {})
}

/// The lines of what was read, the ground's height at points asked of `ground`.
pub fn calculate<E>(input: &Input, ground: impl FnOnce(&[[f64; 2]]) -> Result<Vec<f64>, E>, log: impl Fn(&str)) -> Result<Solved, E> {
    let clip = |pieces: Vec<Piece>| -> Vec<Piece> {
        match &input.window {
            Some((bounds, _)) => pieces.into_iter().flat_map(|p| p.clip(*bounds)).collect(),
            None => pieces,
        }
    };
    let decks = clip(input.rows.iter().flat_map(|r| pieces_of(r, Form::Deck)).collect());
    let bores = clip(input.rows.iter().flat_map(|r| pieces_of(r, Form::Bore)).collect());
    log(&format!("{} deck pieces, {} bore pieces", decks.len(), bores.len()));
    if decks.is_empty() && bores.is_empty() {
        return Ok(Solved::default());
    }

    // decks, then bores, then the ways they carry on along
    let n_decks = decks.len();
    let mut pieces: Vec<Piece> = decks.into_iter().chain(bores).collect();
    let lines: Vec<Polyline> = pieces.iter().map(|p| Polyline::of(p.points.iter().map(|q| [q[0], q[1]]))).collect();
    let crossings = relate::decks_crossing(&pieces.iter().collect::<Vec<_>>(), &lines);
    let mut needs = needs_of(input, &pieces[..n_decks], &lines[..n_decks]);
    let ways = clip(approach::continuations(input, &pieces, &grounded(input, &pieces)));
    log(&format!("{} stretches of way carried on along", ways.len()));
    pieces.extend(ways);
    needs.resize(pieces.len(), Vec::new());

    let (mut over, mut tied) = (Vec::new(), 0);
    for c in crossings {
        over.push(match (c.upper < n_decks, c.lower < n_decks) {
            (true, true) => Crossing {
                far: c.upper,
                far_at: pieces[c.upper].to_m(c.upper_at),
                near: c.lower,
                near_at: pieces[c.lower].to_m(c.lower_at),
                gap: clearance_m(kind_of(&pieces[c.lower])) + DECK_THICKNESS_M,
            },
            (false, false) => Crossing {
                far: c.lower,
                far_at: pieces[c.lower].to_m(c.lower_at),
                near: c.upper,
                near_at: pieces[c.upper].to_m(c.upper_at),
                gap: -(headroom_m(kind_of(&pieces[c.lower])) + BORE_COVER_M),
            },
            _ => continue,
        });
        tied += usize::from(c.tied);
    }
    log(&format!(
        "{} ways and waters under decks; {} decks over decks or bores under bores, {} ordered by id where layers tie",
        needs.iter().map(Vec::len).sum::<usize>(),
        over.len(),
        tied
    ));

    let beside = side_by_side(&pieces[..n_decks]);
    let side = Side::of(&pieces, &beside);
    let all: Vec<[f64; 2]> = side.graphs.iter().flat_map(|g| g.at.iter().copied()).collect();
    let mut flat = ground(&all)?.into_iter();
    let grounds: Vec<Vec<f64>> = side.graphs.iter().map(|g| flat.by_ref().take(g.at.len()).collect()).collect();
    log(&format!("{} structures, {} pairs of decks side by side, {} points", side.structures.len(), beside.len(), all.len()));

    let (tops, forced) = side.solve(&grounds, &needs, &over);
    log(&format!("{forced} points steeper than the preferred grade"));
    let out = side.out(&grounds, &tops);
    let count = |f: Form| out.rows.iter().filter(|d| d.form == f.byte()).count();
    log(&format!("{} lines ({} deck, {} bore)", out.rows.len(), count(Form::Deck), count(Form::Bore)));
    Ok(out)
}

/// How far a bound reaches: half a page's side less what is drawn past one,
/// since what a point reads is a bound's reach from a bound's reach, so a
/// page's points read nothing past the pages around it.
fn halo_m(y_db: f64) -> f64 {
    (f64::from(PAGE_CELLS) * U_PER_CELL - DRAWN_REACH_U) / 2.0 / u_per_m(from_db(y_db))
}

fn kind_of(p: &Piece) -> u8 {
    segment_type(&p.subtype, p.class.as_deref())
}

/// `far` stands `gap` over `near`, under where it is negative.
struct Crossing {
    far: usize,
    far_at: f64,
    near: usize,
    near_at: f64,
    gap: f64,
}

/// Structures and the ways they carry on along, each a graph of its pieces' points.
struct Side<'a> {
    pieces: &'a [Piece],
    structures: Vec<Vec<usize>>,
    graphs: Vec<Graph>,
    place: Vec<(usize, usize)>,
}

impl<'a> Side<'a> {
    fn of(pieces: &'a [Piece], beside: &[structure::Beside]) -> Side<'a> {
        let structures = group(pieces, beside);
        let graphs = structures.iter().map(|m| Graph::build(pieces, m, beside)).collect();
        let mut place = vec![(0usize, 0usize); pieces.len()];
        for (s, members) in structures.iter().enumerate() {
            for (k, &m) in members.iter().enumerate() {
                place[m] = (s, k);
            }
        }
        Side { pieces, structures, graphs, place }
    }

    /// Surfaces in z, and the forced count. Twice: crossings read the first
    /// pass's surfaces.
    fn solve(&self, ground: &[Vec<f64>], needs: &[Vec<Need>], crossings: &[Crossing]) -> (Vec<Vec<f64>>, usize) {
        let mut tops: Vec<Vec<f64>> = ground.to_vec();
        let mut forced = 0;
        for pass in 0..2 {
            forced = 0;
            let mut next = Vec::with_capacity(self.structures.len());
            for (s, members) in self.structures.iter().enumerate() {
                let (g, z) = (&self.graphs[s], &ground[s]);
                let (mut floor, mut ceiling) = (vec![f64::NEG_INFINITY; z.len()], vec![f64::INFINITY; z.len()]);
                for (k, &m) in members.iter().enumerate() {
                    let piece = &self.pieces[m];
                    for &(node, _) in &g.along[k] {
                        let n = node as usize;
                        match (piece.approach, piece.form) {
                            (true, _) => {}
                            // and a cell of air under the slab, so a deck stands off the ground
                            (false, Form::Deck) => floor[n] = floor[n].max(z[n] + DECK_THICKNESS_M + U_PER_CELL / u_per_m(from_db(g.at[n][1]))),
                            (false, Form::Bore) => ceiling[n] = ceiling[n].min(z[n] - headroom_m(kind_of(piece)) - BORE_COVER_M),
                        }
                    }
                    for n in needs.get(m).into_iter().flatten() {
                        for node in nodes_within(g, k, n.from, n.to) {
                            floor[node] = floor[node].max(z[node] + n.under.headroom(n.to - n.from) + DECK_THICKNESS_M);
                        }
                    }
                }
                if pass == 1 {
                    for c in crossings.iter().filter(|c| self.place[c.far].0 == s) {
                        let (ns, nk) = self.place[c.near];
                        let near = top_along(&self.graphs[ns], &tops[ns], nk, c.near_at) + c.gap;
                        if let Some(node) = g.nearest(self.place[c.far].1, c.far_at) {
                            let n = node as usize;
                            match c.gap > 0.0 {
                                true => floor[n] = floor[n].max(near),
                                false => ceiling[n] = ceiling[n].min(near),
                            }
                        }
                    }
                }
                // a way's free end is where it carries on at the ground, a box's cut aside
                for (node, end) in &g.free_ends {
                    let n = *node as usize;
                    if !floor[n].is_finite() && !ceiling[n].is_finite() && !matches!(end, End::Clip(..)) {
                        (floor[n], ceiling[n]) = (z[n], z[n]);
                    }
                }
                let profile = solve_profile(&Problem {
                    adj: &g.adj,
                    ground: z,
                    floor: &floor,
                    ceiling: &ceiling,
                    halo: &g.at.iter().map(|a| halo_m(a[1])).collect::<Vec<_>>(),
                });
                forced += profile.forced.iter().filter(|&&f| f).count();
                next.push(profile.top);
            }
            tops = next;
        }
        (tops, forced)
    }

    /// A structure's pieces whole; a way's as its runs off the ground, a deck
    /// over it and a bore under it, split where the surface meets the ground,
    /// so one run ends where the next or the ground begins. A bore no way
    /// reaches is a subway's: carried under the ground, and no line drawn.
    fn out(&self, ground: &[Vec<f64>], tops: &[Vec<f64>]) -> Solved {
        let mut out = Solved::default();
        let reached: Vec<bool> = self.structures.iter().map(|m| m.iter().any(|&i| self.pieces[i].approach)).collect();
        for (m, piece) in self.pieces.iter().enumerate() {
            let (s, k) = self.place[m];
            let (g, top, z) = (&self.graphs[s], &tops[s], &ground[s]);
            // where along, in metres; the surface there; how far over the ground
            let points: Vec<(f64, G3857)> = g.along[k].iter().map(|&(n, d)| (d, [g.at[n as usize][0], g.at[n as usize][1], top[n as usize]])).collect();
            let over: Vec<f64> = g.along[k].iter().map(|&(n, _)| top[n as usize] - z[n as usize]).collect();
            let mut emit = |run: &[(f64, G3857)], form: Form, drawn: bool| {
                if run.len() < 2 {
                    return;
                }
                let (a, b) = (piece.on_way(run[0].0), piece.on_way(run[run.len() - 1].0));
                out.carried.push(Carried {
                    gers_id: piece.gers_id.clone(),
                    range: [a.min(b), a.max(b)],
                    form,
                });
                if drawn {
                    out.rows.push(DeckRow {
                        gers_id: piece.gers_id.clone(),
                        subtype: piece.subtype.clone(),
                        class: piece.class.clone(),
                        width_m: piece.width_m,
                        form: form.byte(),
                        points: run.iter().map(|&(_, p)| p).collect(),
                    });
                }
            };
            if !piece.approach {
                emit(&points, piece.form, piece.form == Form::Deck || reached[s]);
                continue;
            }

            // over, on or under the ground by whole cells, as `Over::occupies` reads it
            let cell_per_m = u_per_m(from_db(piece.points[0][1])) / U_PER_CELL;
            let side: Vec<i8> = over.iter().map(|h| round(h * cell_per_m)).map(|c| i8::from(c > 0.0) - i8::from(c < 0.0)).collect();
            let form = |s: i8| [Some(Form::Bore), None, Some(Form::Deck)][(s + 1) as usize];

            // runs of one side, each ending where the surface meets the ground
            let mut run = vec![points[0]];
            for i in 1..points.len() {
                let (s0, s1) = (side[i - 1], side[i]);
                if s0 == s1 {
                    run.push(points[i]);
                    continue;
                }
                let meet = match (s0, s1) {
                    (0, _) => points[i - 1],
                    (_, 0) => points[i],
                    _ => {
                        let ((d0, p0), (d1, p1), t) = (points[i - 1], points[i], over[i - 1] / (over[i - 1] - over[i]));
                        (d0 + (d1 - d0) * t, std::array::from_fn(|c| p0[c] + (p1[c] - p0[c]) * t))
                    }
                };
                if let Some(f) = form(s0) {
                    run.push(meet);
                    emit(&run, f, true);
                }
                run = if s1 == 0 { vec![meet] } else { vec![meet, points[i]] };
            }
            if let Some(f) = form(side[side.len() - 1]) {
                emit(&run, f, true);
            }
        }
        out
    }
}

/// Ends only one piece reaches, a box's cut aside.
fn free_ends(pieces: &[Piece]) -> Vec<(usize, bool)> {
    let mut count: HashMap<&End, usize> = HashMap::new();
    for end in pieces.iter().flat_map(|p| &p.ends) {
        *count.entry(end).or_default() += 1;
    }
    let free = |&(i, last): &(usize, bool)| count[pieces[i].end(last)] == 1 && !matches!(pieces[i].end(last), End::Clip(..));
    (0..pieces.len()).flat_map(|i| [(i, false), (i, true)]).filter(free).collect()
}

/// Free ends whose way carries on on the ground; a subway's do not.
fn grounded(input: &Input, pieces: &[Piece]) -> Vec<(usize, bool)> {
    let ends = free_ends(pieces);
    let asked: Vec<([f64; 2], &str)> = ends.iter().map(|&(i, last)| (end_of(&pieces[i], last).0, pieces[i].gers_id.as_str())).collect();
    let mut at_ground = vec![false; ends.len()];
    for (point, own_at, way) in relate::connected(&asked, &input.ways) {
        let row = &way.row;
        let step = SIDE_STEP / way.line.length().max(SIDE_STEP);
        let cell = cell_of(way.line.length());
        let off = |at: f64| flag_at(row.flags.as_deref(), UNSEEN_FLAGS, cell, at) || flag_at(row.flags.as_deref(), DECK_FLAGS, cell, at);
        let on_ground = |at: f64| (0.0..=1.0).contains(&at) && !off(at) && value_at(row.levels.as_deref(), at).unwrap_or(0.0) == 0.0;
        at_ground[point] |= on_ground(own_at - step) || on_ground(own_at + step);
    }
    ends.into_iter().zip(at_ground).filter(|&(_, g)| g).map(|(e, _)| e).collect()
}

/// Ways and water under each deck; staged bores come after the decks.
fn needs_of(input: &Input, pieces: &[Piece], lines: &[Polyline]) -> Vec<Vec<Need>> {
    let mut needs: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
    let inside = |at: f64, m: usize| at > AT_END_M && at < pieces[m].to_m(lines[m].length()) - AT_END_M;
    for mut c in relate::ways_crossing(lines, &input.ways).into_iter().filter(|c| pieces[c.piece].gers_id != c.gers_id) {
        c.at = pieces[c.piece].to_m(c.at);
        let off_ground = flag_ranges(c.flags.as_deref(), UNSEEN_FLAGS, c.cell)
            .iter()
            .chain(&flag_ranges(c.flags.as_deref(), DECK_FLAGS, c.cell))
            .any(|&[a, b]| (a..=b).contains(&c.own_at));
        if off_ground || !inside(c.at, c.piece) {
            continue;
        }
        let under = Under::Way(segment_type(&c.subtype, c.class.as_deref()));
        needs[c.piece].push(Need { from: c.at, to: c.at, under });
    }
    let mut water: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
    for w in relate::water_under(lines, &input.water) {
        if let Some(under) = Under::of_water(&w.subtype, w.class.as_deref()) {
            water[w.piece].push(Need {
                from: pieces[w.piece].to_m(w.from),
                to: pieces[w.piece].to_m(w.to),
                under,
            });
        }
    }
    // touching spans of one kind are one
    for (m, mut spans) in water.into_iter().enumerate() {
        spans.sort_by(|a, b| a.from.total_cmp(&b.from).then(a.to.total_cmp(&b.to)));
        for n in spans {
            match needs[m].last_mut() {
                Some(last) if last.under == n.under && n.from <= last.to + AT_END_M => last.to = last.to.max(n.to),
                _ => needs[m].push(n),
            }
        }
    }
    needs
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
