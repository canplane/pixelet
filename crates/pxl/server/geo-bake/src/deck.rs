// pxl-geo-bake/deck.rs

//! Decks and bores. A structure is every piece of one form its ends join,
//! wherever it runs, solved whole once; a page's lines are what of those
//! profiles reaches it, and the ways their ends go on along at the ground.

pub mod approach;
pub mod elevation;
pub mod memo;
pub mod param;
pub mod piece;
pub mod profile;
pub mod relate;
pub mod source;
pub mod structure;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use postgres::Client;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::deck::{clear_cells, clearance_m, headroom_m, Form, BORE_COVER_M, DECK_THICKNESS_M};
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m, G3857};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::PAGE_CELLS;

use crate::terrain::pipeline::overture::between::flag_ranges;
use crate::terrain::pipeline::overture::policy::{segment_type, DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::terrain::repository::{TerrainRepository, DRAWN_REACH_U};
use approach::branches;
use memo::{Profile, Run};
use param::{AT_END_M, RIVER_M, RIVER_MOST_M, RIVER_PER_M, STREAM_M};
use piece::{pieces_of, End, Piece};
use profile::{solve as solve_profile, Problem};
use relate::{Polyline, Water, Way};
use source::Store;

use crate::segment::Segment;
use structure::{group, Graph};

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

/// What the solve leaves a page: its lines, and the ranges of ways they draw,
/// which the ground leaves unpainted. One solve makes both, so the road
/// leaves the ground's paint exactly where its line begins.
#[derive(Default)]
pub struct Solved {
    pub lines: Vec<Line>,
    pub drawn: Vec<Drawn>,
}

/// A line as the solve left it, its Z the road surface in metres.
pub struct Line {
    pub subtype: String,
    pub class: Option<String>,
    pub width_m: f64,
    pub form: Form,
    pub points: Vec<G3857>,
}

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
            asked.insert(end);
        }
    }
    let mut out: Vec<Vec<Piece>> = group(&pieces)
        .into_iter()
        .map(|m| {
            let mut members: Vec<Piece> = m.into_iter().map(|i| pieces[i].clone()).collect();
            members.sort_by(|a, b| a.id().cmp(&b.id()));
            members
        })
        .collect();
    out.sort_by(|a, b| a[0].gers_id.cmp(&b[0].gers_id));
    Ok(out)
}

/// A structure's surface, from the source along all of it: its ground, what
/// passes under it, and, when `held`, the structures of its form it crosses
/// over (a deck) or under (a bore), each as solved free of crossings.
pub fn solve_structure(r: &mut Reader<impl Store>, members: &[Piece], held: bool) -> Result<Profile, String> {
    if let Some(p) = memo::read(r.memo, members, held) {
        return Ok(p);
    }
    let form = members[0].form;
    let all: Vec<usize> = (0..members.len()).collect();
    let g = Graph::build(members, &all);
    let pages: Vec<IVec> = g.at.iter().map(|&p| elevation::page_of(p)).collect::<BTreeSet<_>>().into_iter().collect();
    let input = source::read_pages(r.client, r.repo, &pages, r.store)?;
    let z = elevation::sample(&input.ground, &g.at)?;
    let lines: Vec<Polyline> = members.iter().map(|p| Polyline::of(p.points.iter().map(|q| [q[0], q[1]]))).collect();

    let (mut floor, mut ceiling) = (vec![f64::NEG_INFINITY; z.len()], vec![f64::INFINITY; z.len()]);
    for (k, piece) in members.iter().enumerate() {
        for &(node, _) in &g.along[k] {
            let n = node as usize;
            match form {
                // and a cell of air under the slab, so a deck stands off the ground
                Form::Deck => floor[n] = floor[n].max(z[n] + DECK_THICKNESS_M + U_PER_CELL / u_per_m(from_db(g.at[n][1]))),
                Form::Bore => ceiling[n] = ceiling[n].min(z[n] - clear_m(headroom_m(kind_of(piece)), g.at[n][1]) - BORE_COVER_M),
                Form::Grade => unreachable!("a structure's piece is a deck or a bore"),
            }
        }
    }
    let base: Vec<f64> = (0..z.len()).map(|n| if floor[n].is_finite() { floor[n] } else { ceiling[n] }).collect();
    if form == Form::Deck {
        for (k, needs) in needs_of(&input.ways, &input.water, members, &lines).iter().enumerate() {
            for n in needs {
                for node in nodes_within(&g, k, n.from, n.to) {
                    floor[node] = floor[node].max(z[node] + clear_m(n.under.headroom(n.to - n.from), g.at[node][1]) + DECK_THICKNESS_M);
                }
            }
        }
    }
    if held {
        for c in crossings(r, members, &lines, &input.rows)? {
            if let Some(node) = g.nearest(c.far, c.far_at) {
                let n = node as usize;
                match c.gap > 0.0 {
                    true => floor[n] = floor[n].max(c.near_top + c.gap),
                    false => ceiling[n] = ceiling[n].min(c.near_top + c.gap),
                }
            }
        }
    }

    // a structure runs on the line between its ends, each held at its form's bound
    let mut ends = vec![false; z.len()];
    for (node, _) in &g.free_ends {
        ends[*node as usize] = true;
    }
    let top = solve_profile(&Problem {
        adj: &g.adj,
        ground: &z,
        floor: &floor,
        ceiling: &ceiling,
        base: &base,
        ends: &ends,
        halo: &g.at.iter().map(|a| halo_m(a[1])).collect::<Vec<_>>(),
    });
    let runs = members
        .iter()
        .zip(&g.along)
        .map(|(p, along)| Run {
            gers_id: p.gers_id.clone(),
            start: p.start.to_bits(),
            points: along.iter().map(|&(n, d)| [d, g.at[n as usize][0], g.at[n as usize][1], top[n as usize]]).collect(),
            ground: [z[along[0].0 as usize], z[along[along.len() - 1].0 as usize]],
        })
        .collect();
    let profile = Profile { runs };
    memo::write(r.memo, members, held, &profile)?;
    Ok(profile)
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
        let near_piece = pieces[near];
        let at = match solved.iter().position(|(s, _)| s.iter().any(|p| p.id() == near_piece.id())) {
            Some(at) => at,
            None => {
                let structure = whole(r.client, vec![near_piece.clone()], form)?.into_iter().next().expect("a piece is in a structure");
                let profile = solve_structure(r, &structure, false)?;
                solved.push((structure, profile));
                solved.len() - 1
            }
        };
        let (structure, profile) = &solved[at];
        let k = structure.iter().position(|p| p.id() == near_piece.id()).expect("the piece is its structure's");
        let gap = match form {
            Form::Deck => clear_m(clearance_m(kind_of(near_piece)), near_piece.points[0][1]) + DECK_THICKNESS_M,
            _ => -(clear_m(headroom_m(kind_of(pieces[far])), pieces[far].points[0][1]) + BORE_COVER_M),
        };
        out.push(Crossing {
            far,
            far_at: pieces[far].to_m(far_at),
            near_top: profile.runs[k].top_along(near_piece.to_m(near_at)),
            gap,
        });
    }
    Ok(out)
}

/// How far a bound's bend reaches along a structure: half a page's side less
/// what is drawn past one.
fn halo_m(y_db: f64) -> f64 {
    (f64::from(PAGE_CELLS) * U_PER_CELL - DRAWN_REACH_U) / 2.0 / u_per_m(from_db(y_db))
}

/// What `m` metres clear takes in whole cells at `y_db`, back in metres.
fn clear_m(m: f64, y_db: f64) -> f64 {
    let cell_per_m = u_per_m(from_db(y_db)) / U_PER_CELL;
    f64::from(clear_cells(m, cell_per_m)) / cell_per_m
}

fn kind_of(p: &Piece) -> u8 {
    segment_type(&p.subtype, p.class.as_deref())
}

/// Ends only one piece reaches.
fn free_ends(pieces: &[Piece]) -> Vec<(usize, bool)> {
    let mut count: HashMap<&End, usize> = HashMap::new();
    for end in pieces.iter().flat_map(|p| &p.ends) {
        *count.entry(end).or_default() += 1;
    }
    let free = |&(i, last): &(usize, bool)| count[pieces[i].end(last)] == 1;
    (0..pieces.len()).flat_map(|i| [(i, false), (i, true)]).filter(free).collect()
}

/// Ways and water under each deck.
fn needs_of(ways: &[Way], water_rows: &[Water], pieces: &[Piece], lines: &[Polyline]) -> Vec<Vec<Need>> {
    let mut needs: Vec<Vec<Need>> = vec![Vec::new(); pieces.len()];
    let inside = |at: f64, m: usize| at > AT_END_M && at < pieces[m].to_m(lines[m].length()) - AT_END_M;
    for mut c in relate::ways_crossing(lines, ways).into_iter().filter(|c| pieces[c.piece].gers_id != c.gers_id) {
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
    for w in relate::water_under(lines, water_rows) {
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
