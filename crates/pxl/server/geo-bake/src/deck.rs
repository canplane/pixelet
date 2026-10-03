// pxl-geo-bake/deck.rs

//! Decks and bores, one solve in `q = sign · z`: over everything for a
//! published bake, over what a page reaches when it is baked on demand.

pub mod approach;
pub mod elevation;
pub mod param;
pub mod piece;
pub mod profile;
pub mod relate;
pub mod source;
pub mod structure;

use std::collections::HashSet;

use postgres::Client;
use pxl_spec::bake::deck::{clearance_m, headroom_m, Form, BORE_COVER_M, DECK_THICKNESS_M};
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::format::DeckRow;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m};
use pxl_spec::space::pyramid::PAGE_CELLS;

use crate::terrain::repository::DRAWN_REACH_U;
use std::path::Path;

use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::terrain::pipeline::overture::between::{flag_at, flag_ranges, value_at};
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

/// Every bridge and tunnel into `overture_deck`, for a published bake.
pub fn solve(client: &mut Client, url: &str, log: impl Fn(&str)) -> Result<(), postgres::Error> {
    let input = source::read_all(client)?;
    let out = calculate(&input, |at| elevation::surface(client, url, at), &log)?;
    source::write(client, &out)?;
    log(&format!("{} lines: overture_deck", out.len()));
    Ok(())
}

/// A page's lines, from what lies in it and the eight pages around it.
pub fn solve_page(client: &mut Client, page: IVec, memo: &Path) -> Result<Vec<DeckRow>, String> {
    let input = source::read_ring(client, page, memo)?;
    let Some((_, grids)) = &input.window else { unreachable!("a ring is read bounded") };
    let (px, py) = (ivec::X(page), ivec::Y(page));
    let within = |p| {
        let at = elevation::page_of(p);
        ivec::gather(ivec::X(at).clamp(px - 1, px + 1), ivec::Y(at).clamp(py - 1, py + 1), 0)
    };
    calculate(&input, |at| Ok::<_, String>(elevation::sample(grids, at, within)), |_| {})
}

/// The lines of what was read, the ground's height at points asked of `ground`.
pub fn calculate<E>(input: &Input, ground: impl FnOnce(&[[f64; 2]]) -> Result<Vec<f64>, E>, log: impl Fn(&str)) -> Result<Vec<DeckRow>, E> {
    let clip = |pieces: Vec<Piece>| -> Vec<Piece> {
        match &input.window {
            Some((bounds, _)) => pieces.into_iter().flat_map(|p| p.clip(*bounds)).collect(),
            None => pieces,
        }
    };
    let mut decks = clip(input.rows.iter().flat_map(|r| pieces_of(r, Form::Deck)).collect());
    let mut bores = clip(input.rows.iter().flat_map(|r| pieces_of(r, Form::Bore)).collect());
    log(&format!("{} deck pieces, {} bore pieces", decks.len(), bores.len()));
    if decks.is_empty() && bores.is_empty() {
        return Ok(Vec::new());
    }

    let n_decks = decks.len();
    let staged: Vec<&Piece> = decks.iter().chain(&bores).collect();
    let lines: Vec<Polyline> = staged.iter().map(|p| Polyline::of(p.points.iter().map(|q| [q[0], q[1]]))).collect();
    let crossings = relate::decks_crossing(&staged, &lines);
    let mut needs = needs_of(input, &decks, &lines[..n_decks]);
    let portals = portals_of(input, &bores);
    let ramps = clip(approach::approaches(input, &decks, &approach::free_ends(&decks), Form::Ramp));
    let cuts = clip(approach::approaches(input, &bores, &portals, Form::Cut));
    log(&format!("{} portals; {} ramps and {} cuttings graded with their structures", portals.len(), ramps.len(), cuts.len()));
    let mut pinned: HashSet<End> = portals.iter().map(|&(i, last)| bores[i].end(last).clone()).collect();
    pinned.extend(cuts.iter().map(|c| c.end(true).clone()));
    decks.extend(ramps);
    bores.extend(cuts);
    needs.resize(decks.len(), Vec::new());

    let (mut over_decks, mut over_bores, mut tied) = (Vec::new(), Vec::new(), 0);
    for c in crossings {
        match (c.upper < n_decks, c.lower < n_decks) {
            (true, true) => over_decks.push(Crossing {
                far: c.upper,
                far_at: decks[c.upper].to_m(c.upper_at),
                near: c.lower,
                near_at: decks[c.lower].to_m(c.lower_at),
                gap: clearance_m(kind_of(&decks[c.lower])) + DECK_THICKNESS_M,
            }),
            (false, false) => {
                let (upper, lower) = (c.upper - n_decks, c.lower - n_decks);
                over_bores.push(Crossing {
                    far: lower,
                    far_at: bores[lower].to_m(c.lower_at),
                    near: upper,
                    near_at: bores[upper].to_m(c.upper_at),
                    gap: headroom_m(kind_of(&bores[lower])) + BORE_COVER_M,
                });
            }
            _ => continue,
        }
        tied += usize::from(c.tied);
    }
    log(&format!(
        "{} ways and waters under decks; {} decks over decks, {} bores over bores, {} ordered by id where layers tie",
        needs.iter().map(Vec::len).sum::<usize>(),
        over_decks.len(),
        over_bores.len(),
        tied
    ));

    let beside = side_by_side(&decks);
    let deck_side = Side::of(&decks, &beside);
    let bore_side = Side::of(&bores, &[]);
    let all: Vec<[f64; 2]> = deck_side.graphs.iter().chain(&bore_side.graphs).flat_map(|g| g.at.iter().copied()).collect();
    let mut flat = ground(&all)?.into_iter();
    let mut ground_of = |side: &Side| -> Vec<Vec<f64>> { side.graphs.iter().map(|g| flat.by_ref().take(g.at.len()).collect()).collect() };
    let (deck_ground, bore_ground) = (ground_of(&deck_side), ground_of(&bore_side));
    let points = deck_side.graphs.iter().chain(&bore_side.graphs).map(|g| g.at.len()).sum::<usize>();
    log(&format!(
        "{} structures, {} pairs of decks side by side, {} points",
        deck_side.structures.len() + bore_side.structures.len(),
        beside.len(),
        points
    ));

    let (deck_tops, forced) = deck_side.solve(1.0, &deck_ground, |_| DECK_THICKNESS_M, &needs, &over_decks, |_| true);
    log(&format!("deck: {forced} points steeper than the preferred grade"));
    let bore_shell = |p: &Piece| headroom_m(kind_of(p)) + BORE_COVER_M;
    let (bore_tops, forced) = bore_side.solve(-1.0, &bore_ground, bore_shell, &[], &over_bores, |end| pinned.contains(end));
    log(&format!("bore: {forced} points steeper than the preferred grade"));

    let mut out = deck_side.out(&deck_ground, &deck_tops, 1.0);
    out.extend(bore_side.out(&bore_ground, &bore_tops, -1.0));
    let unpinned = bore_tops.iter().filter(|t| t.is_none()).count();
    let count = |f: Form| out.iter().filter(|d| d.form == f.byte()).count();
    log(&format!(
        "{} lines in {} structures ({} deck, {} ramp, {} bore, {} cut); {unpinned} bore structures with no portal left out",
        out.len(),
        deck_side.structures.len() + bore_side.structures.len(),
        count(Form::Deck),
        count(Form::Ramp),
        count(Form::Bore),
        count(Form::Cut)
    ));
    Ok(out)
}

/// How far a need reaches: a page's side less what is drawn past one, so a
/// page's points read nothing past the pages around it.
fn halo_m(y_db: f64) -> f64 {
    (f64::from(PAGE_CELLS) * U_PER_CELL - DRAWN_REACH_U) / u_per_m(from_db(y_db))
}

fn kind_of(p: &Piece) -> u8 {
    segment_type(&p.subtype, p.class.as_deref())
}

/// `far` stands `gap` past `near`, away from the ground.
struct Crossing {
    far: usize,
    far_at: f64,
    near: usize,
    near_at: f64,
    gap: f64,
}

/// One form's structures, each a graph of its pieces' points.
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

    /// Surfaces in `q`, `None` with no pinned end, and the forced count.
    /// Twice: crossings read the first pass's surfaces.
    fn solve(&self, sign: f64, ground: &[Vec<f64>], shell: impl Fn(&Piece) -> f64, needs: &[Vec<Need>], crossings: &[Crossing], pinned: impl Fn(&End) -> bool) -> (Vec<Option<Vec<f64>>>, usize) {
        let q_ground: Vec<Vec<f64>> = ground.iter().map(|g| g.iter().map(|v| sign * v).collect()).collect();
        let carried: Vec<Vec<bool>> = self.structures.iter().zip(&self.graphs).map(|(m, g)| carried_of(self.pieces, m, g)).collect();
        let mut tops: Vec<Option<Vec<f64>>> = q_ground.iter().map(|g| Some(g.clone())).collect();
        let mut forced = 0;
        for pass in 0..2 {
            forced = 0;
            let mut next = Vec::with_capacity(self.structures.len());
            for (s, members) in self.structures.iter().enumerate() {
                let (g, qg) = (&self.graphs[s], &q_ground[s]);
                if sign < 0.0 && !g.free_ends.iter().any(|(_, end)| pinned(end)) {
                    next.push(None);
                    continue;
                }
                let mut need = qg.clone();
                for (k, &m) in members.iter().enumerate() {
                    let piece = &self.pieces[m];
                    if !piece.approach {
                        for &(node, _) in &g.along[k] {
                            need[node as usize] = need[node as usize].max(qg[node as usize] + shell(piece));
                        }
                    }
                    for n in needs.get(m).into_iter().flatten() {
                        for node in nodes_within(g, k, n.from, n.to) {
                            need[node] = need[node].max(qg[node] + n.under.headroom(n.to - n.from) + shell(piece));
                        }
                    }
                }
                if pass == 1 {
                    for c in crossings.iter().filter(|c| self.place[c.far].0 == s) {
                        let (ns, nk) = self.place[c.near];
                        let Some(near_tops) = &tops[ns] else { continue };
                        let near = top_along(&self.graphs[ns], near_tops, nk, c.near_at);
                        if let Some(node) = g.nearest(self.place[c.far].1, c.far_at) {
                            need[node as usize] = need[node as usize].max(near + c.gap);
                        }
                    }
                }
                let approaches: Vec<&[(u32, f64)]> = members.iter().enumerate().filter(|&(_, &m)| self.pieces[m].approach).map(|(k, _)| &g.along[k][..]).collect();
                let profile = solve_profile(&Problem {
                    adj: &g.adj,
                    need: &need,
                    ground: qg,
                    carried: &carried[s],
                    approaches: &approaches,
                    halo: halo_m(g.at[0][1]),
                });
                forced += profile.forced.iter().filter(|&&f| f).count();
                next.push(Some(profile.top));
            }
            tops = next;
        }
        (tops, forced)
    }

    /// In z; an approach only while off the ground.
    fn out(&self, ground: &[Vec<f64>], tops: &[Option<Vec<f64>>], sign: f64) -> Vec<DeckRow> {
        let mut out = Vec::new();
        for (m, piece) in self.pieces.iter().enumerate() {
            let (s, k) = self.place[m];
            let (Some(top), g) = (&tops[s], &self.graphs[s]) else { continue };
            let nodes = on_structure(piece, &g.along[k], |n| top[n] > sign * ground[s][n]);
            if nodes.len() >= 2 {
                out.push(DeckRow {
                    gers_id: piece.gers_id.clone(),
                    subtype: piece.subtype.clone(),
                    class: piece.class.clone(),
                    width_m: piece.width_m,
                    form: piece.form.byte(),
                    points: nodes.iter().map(|&(n, _)| [g.at[n as usize][0], g.at[n as usize][1], sign * top[n as usize]]).collect(),
                });
            }
        }
        out
    }
}

/// Tunnel ends whose way carries on at the surface; a subway's do not.
fn portals_of(input: &Input, tunnels: &[Piece]) -> Vec<(usize, bool)> {
    let ends = approach::free_ends(tunnels);
    let asked: Vec<([f64; 2], &str)> = ends.iter().map(|&(i, last)| (end_of(&tunnels[i], last).0, tunnels[i].gers_id.as_str())).collect();
    let mut portal = vec![false; ends.len()];
    for (point, own_at, way) in relate::connected(&asked, &input.ways) {
        let row = &way.row;
        let step = SIDE_STEP / way.line.length().max(SIDE_STEP);
        let at_surface = |at: f64| (0.0..=1.0).contains(&at) && !flag_at(row.flags.as_deref(), UNSEEN_FLAGS, at) && value_at(row.levels.as_deref(), at).unwrap_or(0.0) >= 0.0;
        if at_surface(own_at - step) || at_surface(own_at + step) {
            portal[point] = true;
        }
    }
    ends.into_iter().zip(portal).filter(|&(_, p)| p).map(|(e, _)| e).collect()
}

/// The structure's own points; a joint is one.
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

/// Ways and water under each deck; staged bores come after the decks.
fn needs_of(input: &Input, pieces: &[Piece], lines: &[Polyline]) -> Vec<Vec<Need>> {
    let mut needs: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
    let inside = |at: f64, m: usize| at > AT_END_M && at < pieces[m].to_m(lines[m].length()) - AT_END_M;
    for mut c in relate::ways_crossing(lines, &input.ways).into_iter().filter(|c| pieces[c.piece].gers_id != c.gers_id) {
        c.at = pieces[c.piece].to_m(c.at);
        let off_ground = flag_ranges(c.flags.as_deref(), UNSEEN_FLAGS)
            .iter()
            .chain(&flag_ranges(c.flags.as_deref(), DECK_FLAGS))
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
