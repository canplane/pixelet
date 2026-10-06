// pxl-geo-bake/deck/arnis.rs

//! Bridges and tunnels for one tile, heights as Arnis (OSM to Minecraft,
//! Apache-2.0) solves them in its element processing for bridges, highways
//! and railways, without the blocks it places: a road bridge's deck is the
//! least concave line over what it must clear, a rail viaduct one level with
//! ramps, a road tunnel as deep as its cover asks and reached by its approach
//! roads sinking into the portal, a rail tunnel at a fixed depth under the
//! terrain. Arnis's numbers are metres (a block a metre upright on the Earth,
//! as its celestial bodies say) taken into the tile's cells, half to even.
//! What is read is the tile and as much of its 1-ring as the structures
//! touching it reach, and no further.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Instant;

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::deck::{footprint as line_reach, Form};
use pxl_spec::space::adapter::epsg3857::{from_db, get_tile_bbox, to_db, u_per_m};
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_core::math::scalar::round;

use crate::deck::elevation::{sample_cells, tile_of};
use crate::deck::piece::{pieces_of, pieces_over, End, Piece};
use crate::deck::source::{ring, Store};
use crate::deck::way::{Polyline, Way};
use crate::deck::{Drawn, Line, Reader, Solved};
use crate::segment::Segment;
use crate::terrain::bake::Surface;
use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, length_of, value_at, without};
use crate::terrain::pipeline::overture::policy::DECK_FLAGS;
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::wkb::wkb_to_geometries;

// ---- Arnis's numbers, in metres or as ratios, under its names ----
const LAYER_HEIGHT_STEP_M: f64 = 6.0;
const FLAT_TERRAIN_DIP_THRESHOLD_M: f64 = 4.0;
const SHORT_BRIDGE_LENGTH_M: f64 = 30.0;
const DUAL_CARRIAGEWAY_MAX_DISTANCE_M: f64 = 12.0;
const DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG: f64 = 20.0;
const SIDE_DECK_EDGE_GAP_M: f64 = 2.0;
const ROAD_HEADROOM_M: f64 = 6.0;
const PATH_HEADROOM_M: f64 = 5.0;
const RAIL_HEADROOM_M: f64 = 8.0;
const RIVER_HEADROOM_M: f64 = 4.0;
const STREAM_HEADROOM_M: f64 = 2.0;
const STACKED_DECK_HEADROOM_M: f64 = 6.0;
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
const TUNNEL_CEIL_OFFSET_M: f64 = 5.0;
const TUNNEL_COVER_DROP_M: f64 = 7.0;
const TUNNEL_ROOF_COVER_M: f64 = TUNNEL_CEIL_OFFSET_M + 1.0;
const TUNNEL_RAMP_STEP: i32 = 1;
const TUNNEL_RAMP_RUN: i32 = 3;
const TUNNEL_LAYER_DROP_M: f64 = 7.0;
const TUNNEL_MAX_LAYERS_BELOW: i64 = 8;
const TUNNEL_APPROACH_WAYS: usize = 8;
const RAIL_TUNNEL_DEPTH_M: f64 = 3.0;
const RAIL_INTERIOR_HEIGHT_M: f64 = 4.0;
/// A block, as a length: what Arnis adds or takes by one.
const BLOCK_M: f64 = 1.0;

/// What the solve did, and how long each part took.
#[derive(Default)]
pub struct Report {
    pub bridges: usize,
    pub rail_bridges: usize,
    pub tunnels: usize,
    pub rail_tunnels: usize,
    pub ramps: usize,
    pub approaches: usize,
    pub tiles_read: usize,
    pub times: Times,
}

/// Seconds spent reading and solving.
#[derive(Default, Clone, Copy)]
pub struct Times {
    pub read: f64,
    pub solve: f64,
}

/// A piece by identity: its way and where on it it starts.
type Id = (String, u64);

/// The tile's scale: Arnis's metres in its cells.
#[derive(Clone, Copy)]
struct Units {
    cell_per_m: f64,
}

impl Units {
    fn c(self, m: f64) -> i32 {
        round(m * self.cell_per_m) as i32
    }
}

/// What the search has read: the ring's tiles it reached, their ground, every
/// way on them, their water, and their structures' pieces.
struct Source {
    ring: BTreeSet<IVec>,
    loaded: BTreeSet<IVec>,
    ground: HashMap<IVec, Surface>,
    ways: BTreeMap<String, Way>,
    water: Vec<(Option<String>, String, Vec<[f64; 2]>)>,
    water_seen: HashSet<Vec<u8>>,
    pieces: BTreeMap<Id, Sampled>,
    by_end: HashMap<End, Vec<Id>>,
    /// Every way by its first and last vertex.
    way_ends: HashMap<End, Vec<String>>,
    read: f64,
}

/// A piece, and once wanted its samples a cell apart along its way inside the ring.
struct Sampled {
    piece: Piece,
    n: usize,
    samples: Option<BTreeMap<usize, ([f64; 2], i32)>>,
}

/// A stretch of a piece inside the ring: what Arnis takes for a way, its
/// cells a cell apart.
struct Run {
    piece: Piece,
    k0: usize,
    /// Cells: DB coordinates, the ring's cell coordinates, the terrain.
    at: Vec<[f64; 2]>,
    cell: Vec<[f64; 2]>,
    terrain: Vec<i32>,
    ends: [End; 2],
    /// Whether an end is where the ring cut the piece.
    clipped: [bool; 2],
    /// Every node it passes, ends included.
    nodes: Vec<End>,
    half: i32,
    width_cells: f64,
}

impl Run {
    /// A railway's, not a road's: Arnis solves the two apart.
    fn rail(&self) -> bool {
        self.piece.subtype == "rail"
    }
}

/// A way at grade made to stand off it, a bridge's ramp or a tunnel's
/// approach: a grade line at `points`.
fn graded(way: &Way, points: Vec<[f64; 3]>, units: Units) -> (Line, Drawn) {
    (
        Line {
            subtype: way.row.subtype.clone(),
            class: way.row.class.clone(),
            width_cells: way_width_m(&way.row) * units.cell_per_m,
            form: Form::Grade,
            points,
            clear: 0,
        },
        Drawn {
            gers_id: way.row.gers_id.clone(),
            range: [0.0, 1.0],
        },
    )
}

pub fn solve_tile(r: &mut Reader<impl Store>, tile: IVec) -> Result<(Solved, Report), String> {
    let mut report = Report::default();
    let tiles = ring(tile);
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for &p in &tiles {
        for c in get_tile_bbox(p) {
            for k in 0..2 {
                (lo[k], hi[k]) = (lo[k].min(c[k]), hi[k].max(c[k]));
            }
        }
    }
    let [pmin, pmax] = get_tile_bbox(tile);
    let units = Units {
        cell_per_m: u_per_m(from_db(pmin[1])) / U_PER_CELL,
    };
    let mut src = Source {
        ring: tiles.into_iter().collect(),
        loaded: BTreeSet::new(),
        ground: HashMap::new(),
        ways: BTreeMap::new(),
        water: Vec::new(),
        water_seen: HashSet::new(),
        pieces: BTreeMap::new(),
        by_end: HashMap::new(),
        way_ends: HashMap::new(),
        read: 0.0,
    };
    src.load(r, tile)?;

    // ---- the structures the tile holds and every one they reach: shared ends, decks
    // beside or across them; each read as far as the ring ----
    let own = [[pmin[0], pmin[1]], [pmax[0], pmax[1]]];
    let reach = to_db(f64::from(units.c(DUAL_CARRIAGEWAY_MAX_DISTANCE_M) + 16) * U_PER_CELL);
    let mut scope: BTreeSet<Id> = src.pieces.iter().filter(|(_, s)| s.piece.touches(own)).map(|(id, _)| id.clone()).collect();
    // to a fixed point: a tile read late can hold what one looked at early stands beside
    loop {
        let before = (scope.len(), src.loaded.len());
        for id in scope.clone() {
            src.sample(r, &id, units)?;
            let s = &src.pieces[&id].piece;
            scope.extend(s.ends.iter().flat_map(|e| src.by_end.get(e).into_iter().flatten().cloned()).collect::<Vec<_>>());
            if s.form == Form::Deck {
                let b = grown(bbox_of(s), reach);
                scope.extend(
                    src.pieces
                        .iter()
                        .filter(|(_, o)| o.piece.form == Form::Deck && o.piece.touches(b))
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>(),
                );
            }
        }
        if (scope.len(), src.loaded.len()) == before {
            break;
        }
    }
    let t = Instant::now();
    let runs = runs_of(&src, &scope, lo, units);
    let obstacles = Obstacles::of(&src, lo, units);
    let mut lines: Vec<(Line, Drawn)> = Vec::new();

    // ---- road bridges, lower first; rail bridges over them; then the tunnels ----
    let solved = bridges(&src, &runs, &obstacles, units, &mut lines, &mut report);
    for (q, ys) in &solved.ys {
        lines.push(part_of(&runs[*q], ys, [0, ys.len() - 1], 0));
    }
    for (q, ys) in rail_bridges(&runs, &obstacles, &solved.decks, units, &mut report) {
        lines.push(part_of(&runs[q], &ys, [0, ys.len() - 1], 0));
    }
    let ceiling = (units.c(TUNNEL_CEIL_OFFSET_M) - units.c(BLOCK_M)).clamp(0, 255) as u8;
    for (q, ys, covered) in road_tunnels(&src, &runs, &solved.ramp_ways, units, &mut lines, &mut report) {
        // a stretch under cover keeps its ceiling; one open to the sky is cut to the surface
        for (stretch, open) in split_by(&covered) {
            lines.push(part_of(&runs[q], &ys, stretch, if open { u8::MAX } else { ceiling }));
        }
    }
    for q in (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Bore && runs[q].rail()) {
        // the floor under the interior, under the ceiling, under the cover (Arnis `generate_rail_tunnel_shell`)
        let depth = units.c(RAIL_TUNNEL_DEPTH_M + RAIL_INTERIOR_HEIGHT_M + BLOCK_M);
        let ys: Vec<i32> = runs[q].terrain.iter().map(|t| t - depth).collect();
        lines.push(part_of(&runs[q], &ys, [0, ys.len() - 1], units.c(RAIL_INTERIOR_HEIGHT_M).clamp(0, 255) as u8));
        report.rail_tunnels += 1;
    }
    report.times.solve = t.elapsed().as_secs_f64();
    report.times.read = src.read;
    report.tiles_read = src.loaded.len();
    let mut out = Solved::default();
    for (line, drawn) in lines.into_iter().filter(|(l, _)| l.points.len() >= 2) {
        out.lines.push(line);
        out.drawn.push(drawn);
    }
    Ok((out, report))
}

fn bbox_of(p: &Piece) -> [[f64; 2]; 2] {
    let mut b = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    for q in &p.points {
        for k in 0..2 {
            (b[0][k], b[1][k]) = (b[0][k].min(q[k]), b[1][k].max(q[k]));
        }
    }
    b
}

fn grown(b: [[f64; 2]; 2], by: f64) -> [[f64; 2]; 2] {
    [[b[0][0] - by, b[0][1] - by], [b[1][0] + by, b[1][1] + by]]
}

fn is_subway(s: &Segment) -> bool {
    s.subtype == "rail" && s.class.as_deref() == Some("subway")
}

impl Source {
    /// Reads a tile of the ring once; a tile past it is never read.
    fn load(&mut self, r: &mut Reader<impl Store>, p: IVec) -> Result<(), String> {
        if !self.ring.contains(&p) || self.loaded.contains(&p) {
            return Ok(());
        }
        let t = Instant::now();
        self.loaded.insert(p);
        let rows = r.store.tile(r.client, r.repo, p)?;
        self.ground.insert(p, rows.surface());
        let step = to_db(U_PER_CELL);
        for s in rows.ways() {
            if self.ways.contains_key(&s.gers_id) || s.subtype == "water" {
                continue;
            }
            let mut pieces = Vec::new();
            if s.flagged() || is_subway(s) {
                pieces.extend(pieces_of(s, Form::Deck));
                // a subway runs underground wherever it is no bridge (Arnis `wants_rail_tunnel`)
                if is_subway(s) {
                    let cell = cell_of(length_of(&s.geom_wkb));
                    pieces.extend(pieces_over(s, Form::Bore, &without(&[[0.0, 1.0]], &flag_ranges(s.flags.as_deref(), DECK_FLAGS, cell))));
                } else {
                    pieces.extend(pieces_of(s, Form::Bore));
                }
            }
            for piece in pieces {
                let id = piece.id();
                for e in &piece.ends {
                    self.by_end.entry(e.clone()).or_default().push(id.clone());
                }
                let n = libm::ceil(piece.length / step).max(1.0) as usize;
                self.pieces.insert(id, Sampled { piece, n, samples: None });
            }
            if let Some(way) = Way::of(s.clone()) {
                for p in [way.line.points[0], way.line.points[way.line.points.len() - 1]] {
                    self.way_ends.entry(End::at(p)).or_default().push(s.gers_id.clone());
                }
                self.ways.insert(s.gers_id.clone(), way);
            }
        }
        for (subtype, class, wkb) in rows.water() {
            if !self.water_seen.insert(wkb.clone()) {
                continue;
            }
            for g in wkb_to_geometries(wkb).into_iter().filter(|g| !g.is_polygon_source && g.points.len() >= 2) {
                self.water.push((class.clone(), subtype.clone(), g.points.iter().map(|p| [p[0], p[1]]).collect()));
            }
        }
        self.read += t.elapsed().as_secs_f64();
        Ok(())
    }

    /// A piece's cells inside the ring, the tiles its footprint reaches read first.
    fn sample(&mut self, r: &mut Reader<impl Store>, id: &Id, units: Units) -> Result<(), String> {
        let s = &self.pieces[id];
        if s.samples.is_some() {
            return Ok(());
        }
        let step = to_db(U_PER_CELL);
        let line = Polyline::of(s.piece.points.iter().map(|p| [p[0], p[1]]));
        let length = line.length();
        let reach = to_db((line_reach(s.piece.width_m * units.cell_per_m) + 16.0) * U_PER_CELL);
        let at: Vec<(usize, [f64; 2])> = (0..=s.n)
            .map(|k| (k, line.at((k as f64 * step).min(length))))
            .filter(|(_, p)| self.ring.contains(&tile_of(*p)))
            .collect();
        let mut wanted: BTreeSet<IVec> = BTreeSet::new();
        for (_, p) in &at {
            for d in [[0.0, 0.0], [reach, 0.0], [-reach, 0.0], [0.0, reach], [0.0, -reach]] {
                wanted.insert(tile_of([p[0] + d[0], p[1] + d[1]]));
            }
        }
        for e in [s.piece.points[0], s.piece.points[s.piece.points.len() - 1]] {
            wanted.insert(tile_of([e[0], e[1]]));
        }
        for p in wanted {
            self.load(r, p)?;
        }
        let t = Instant::now();
        let ts = sample_cells(&self.ground, &at.iter().map(|a| a.1).collect::<Vec<_>>())?;
        self.pieces.get_mut(id).expect("a piece read").samples = Some(at.into_iter().zip(ts).map(|((k, p), t)| (k, (p, t))).collect());
        self.read += t.elapsed().as_secs_f64();
        Ok(())
    }
}

/// Each piece in scope as its stretches inside the ring.
fn runs_of(src: &Source, scope: &BTreeSet<Id>, origin: [f64; 2], units: Units) -> Vec<Run> {
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
            let clipped = [k0 != 0, k1 != s.n];
            let cut = |k: usize| End::Cut(format!("{}@ring", s.piece.gers_id), (k as f64).to_bits());
            let ends = [if clipped[0] { cut(k0) } else { s.piece.ends[0].clone() }, if clipped[1] { cut(k1) } else { s.piece.ends[1].clone() }];
            let mut nodes: Vec<End> = s.piece.points.iter().map(|p| End::at([p[0], p[1]])).collect();
            nodes.extend(ends.iter().cloned());
            out.push(Run {
                piece: s.piece.clone(),
                k0,
                cell: at.iter().map(|p| [from_db(p[0] - origin[0]) / U_PER_CELL, from_db(p[1] - origin[1]) / U_PER_CELL]).collect(),
                at,
                terrain,
                ends,
                clipped,
                nodes,
                half: units.c(s.piece.width_m / 2.0),
                width_cells: s.piece.width_m * units.cell_per_m,
            });
        }
    }
    out
}

fn is_non_vehicular(class: &str) -> bool {
    matches!(class, "footway" | "pedestrian" | "path" | "cycleway" | "bridleway" | "steps")
}

/// Arnis's `effective_layer`: the layer, never below 0; a bridge untagged is 1.
fn effective_layer(r: &Run) -> i64 {
    r.piece.layer.map(|l| l.max(0)).unwrap_or(i64::from(r.piece.form == Form::Deck))
}

/// A way's width at its middle, in ground metres.
fn way_width_m(row: &Segment) -> f64 {
    width_m(&row.subtype, row.class.as_deref(), WayShape::at(row.columns(), 0.5))
}

// ---- what a deck must clear: ways at grade and waterways, bucketed ----

struct Obstacle {
    a: [f64; 2],
    b: [f64; 2],
    reach: f64,
    headroom: i32,
    nodes: Vec<End>,
}

struct Obstacles {
    list: Vec<Obstacle>,
    buckets: HashMap<(i64, i64), Vec<usize>>,
    bucket: f64,
}

impl Obstacles {
    fn of(src: &Source, origin: [f64; 2], units: Units) -> Obstacles {
        let cell = |p: [f64; 2]| [from_db(p[0] - origin[0]) / U_PER_CELL, from_db(p[1] - origin[1]) / U_PER_CELL];
        let mut out = Obstacles {
            list: Vec::new(),
            buckets: HashMap::new(),
            bucket: f64::from(units.c(OBSTACLE_GRID_M).max(1)),
        };
        let add = |out: &mut Obstacles, points: &[[f64; 2]], half: i32, headroom: i32| {
            let nodes: Vec<End> = points.iter().map(|&p| End::at(p)).collect();
            let cells: Vec<[f64; 2]> = points.iter().map(|&p| cell(p)).collect();
            for pair in cells.windows(2) {
                let i = out.list.len();
                out.list.push(Obstacle {
                    a: pair[0],
                    b: pair[1],
                    reach: f64::from(half) + 0.5,
                    headroom,
                    nodes: nodes.clone(),
                });
                let r = f64::from(half + 1);
                let k = |v: f64| (v / out.bucket).floor() as i64;
                for bx in k(pair[0][0].min(pair[1][0]) - r)..=k(pair[0][0].max(pair[1][0]) + r) {
                    for by in k(pair[0][1].min(pair[1][1]) - r)..=k(pair[0][1].max(pair[1][1]) + r) {
                        out.buckets.entry((bx, by)).or_default().push(i);
                    }
                }
            }
        };
        for way in src.ways.values() {
            let row = &way.row;
            // a way off the ground, or flagged anywhere, is no grade a deck clears (Arnis `grade_obstacle`)
            if row.flagged() || value_at(row.levels.as_deref(), 0.5).is_some_and(|l| l != 0.0) {
                continue;
            }
            match row.subtype.as_str() {
                "road" => {
                    let class = row.class.as_deref().unwrap_or("");
                    let headroom = if is_non_vehicular(class) || class == "track" { PATH_HEADROOM_M } else { ROAD_HEADROOM_M };
                    let half = units.c(way_width_m(row) / 2.0);
                    add(&mut out, &way.line.points, half, units.c(headroom));
                }
                "rail" if !is_subway(row) => {
                    let headroom = if row.class.as_deref() == Some("tram") { ROAD_HEADROOM_M } else { RAIL_HEADROOM_M };
                    add(&mut out, &way.line.points, units.c(BLOCK_M), units.c(headroom));
                }
                _ => {}
            }
        }
        for (class, subtype, points) in &src.water {
            let headroom = match class.as_deref().unwrap_or(subtype) {
                "river" | "canal" => RIVER_HEADROOM_M,
                "stream" => STREAM_HEADROOM_M,
                _ => continue,
            };
            add(&mut out, points, 0, units.c(headroom));
        }
        out
    }

    fn at(&self, p: [f64; 2]) -> impl Iterator<Item = &Obstacle> {
        let k = ((p[0] / self.bucket).floor() as i64, (p[1] / self.bucket).floor() as i64);
        self.buckets.get(&k).into_iter().flatten().map(|&i| &self.list[i])
    }
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
type Decks = HashMap<(i64, i64), (i32, i64)>;

struct Bridges {
    ys: Vec<(usize, Vec<i32>)>,
    decks: Decks,
    /// The ways a bridge's ramp took, kept from a tunnel's approach.
    ramp_ways: HashSet<String>,
}

fn cell_key(c: [f64; 2]) -> (i64, i64) {
    (c[0].floor() as i64, c[1].floor() as i64)
}

fn heading_of(r: &Run) -> Option<f64> {
    let (s, e) = (r.cell[0], r.cell[r.cell.len() - 1]);
    let d = [e[0] - s[0], e[1] - s[1]];
    (d != [0.0, 0.0]).then(|| libm::atan2(d[1], d[0]).to_degrees())
}

fn parallel(a: f64, b: f64) -> bool {
    let mut diff = (a - b).abs() % 360.0;
    if diff > 180.0 {
        diff = 360.0 - diff;
    }
    diff <= DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG || (180.0 - diff).abs() <= DUAL_CARRIAGEWAY_HEADING_TOLERANCE_DEG
}

fn lateral(p: [f64; 2], r: &Run) -> f64 {
    r.cell.windows(2).map(|w| dist_to_segment(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}

/// Arnis `side_by_side`: parallel decks forming one, across layers only when they touch.
fn side_by_side(a: &Run, b: &Run, units: Units) -> bool {
    let (Some(ha), Some(hb)) = (heading_of(a), heading_of(b)) else { return false };
    if !parallel(ha, hb) {
        return false;
    }
    let same = effective_layer(a) == effective_layer(b);
    let touching = f64::from(a.half + b.half + 1) + f64::from(units.c(SIDE_DECK_EDGE_GAP_M));
    let max = if same { touching.max(f64::from(units.c(DUAL_CARRIAGEWAY_MAX_DISTANCE_M))) } else { touching };
    let (short, long) = if a.cell.len() <= b.cell.len() { (a, b) } else { (b, a) };
    let n = short.cell.len() - 1;
    let offsets: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0].iter().map(|f| lateral(short.cell[round(f * n as f64) as usize], long)).collect();
    let alongside = offsets.iter().filter(|&&d| d <= max).count();
    if same {
        alongside * 2 > offsets.len()
    } else {
        alongside == offsets.len() && offsets.iter().sum::<f64>() / offsets.len() as f64 >= 1.5
    }
}

struct Uf(Vec<usize>);

impl Uf {
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
    let (Some(&first), Some(&last)) = (us.first(), us.last()) else { return true };
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

/// A way's line from the node it has there, onward.
fn way_from(way: &Way, node: &End) -> Option<Vec<[f64; 2]>> {
    let pts = &way.line.points;
    if &End::at(pts[0]) == node {
        Some(pts.clone())
    } else if &End::at(pts[pts.len() - 1]) == node {
        Some(pts.iter().rev().copied().collect())
    } else {
        None
    }
}

/// Points a cell apart along a line, from its start.
fn cells_along(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let line = Polyline::of(points.iter().copied());
    let step = to_db(U_PER_CELL);
    let n = libm::ceil(line.length() / step).max(1.0) as usize;
    (0..=n).map(|k| line.at((k as f64 * step).min(line.length()))).collect()
}

fn ramp_len(total: f64, units: Units) -> f64 {
    (total * DECK_RAMP_FRACTION).clamp(f64::from(units.c(DECK_RAMP_M[0])), f64::from(units.c(DECK_RAMP_M[1])))
}

fn bridges(src: &Source, runs: &[Run], obstacles: &Obstacles, units: Units, lines: &mut Vec<(Line, Drawn)>, report: &mut Report) -> Bridges {
    let road: Vec<usize> = (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Deck && !runs[q].rail()).collect();
    let mut uf = Uf((0..road.len()).collect());
    // shared ends at one layer, then decks side by side
    let mut by_node: BTreeMap<(&String, u64, u8), Vec<usize>> = BTreeMap::new();
    let node_key = |n: &End| -> (String, u64, u8) {
        match n {
            End::Vertex(x, y) => (x.to_string(), *y, 0),
            End::Cut(g, at) => (g.clone(), *at, 1),
        }
    };
    let keys: Vec<[(String, u64, u8); 2]> = road.iter().map(|&q| [node_key(&runs[q].ends[0]), node_key(&runs[q].ends[1])]).collect();
    for (i, k) in keys.iter().enumerate() {
        for e in k {
            by_node.entry((&e.0, e.1, e.2)).or_default().push(i);
        }
    }
    for members in by_node.values() {
        for (a, &i) in members.iter().enumerate() {
            for &j in &members[a + 1..] {
                if effective_layer(&runs[road[i]]) == effective_layer(&runs[road[j]]) {
                    uf.union(i, j);
                }
            }
        }
    }
    for i in 0..road.len() {
        for j in i + 1..road.len() {
            if uf.find(i) != uf.find(j) && side_by_side(&runs[road[i]], &runs[road[j]], units) {
                uf.union(i, j);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &r) in road.iter().enumerate() {
        groups.entry(uf.find(i)).or_default().push(r);
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().collect();
    groups.sort_by_key(|g| (g.iter().map(|&q| effective_layer(&runs[q])).max().unwrap_or(0), runs[g[0]].piece.id()));

    let mut bridge_nodes: HashMap<&End, Vec<usize>> = HashMap::new();
    for q in (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Deck) {
        for e in &runs[q].ends {
            bridge_nodes.entry(e).or_default().push(q);
        }
    }
    let mut decks: Decks = HashMap::new();
    let mut joints: HashMap<End, i32> = HashMap::new();
    let mut claimed: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for g in &groups {
        report.bridges += 1;
        let mut node_members: HashMap<&End, usize> = HashMap::new();
        let mut group_nodes: HashSet<&End> = HashSet::new();
        for &q in g {
            let seen: HashSet<&End> = runs[q].nodes.iter().collect();
            for n in &seen {
                *node_members.entry(n).or_default() += 1;
            }
            group_nodes.extend(seen);
        }
        let is_boundary = |n: &End| node_members.get(n).copied().unwrap_or(0) <= 1;
        let joins_other = |n: &End| bridge_nodes.get(n).is_some_and(|qs| qs.iter().any(|q| !g.contains(q)));
        let max_layer = g.iter().filter_map(|&q| runs[q].piece.layer).map(|l| l.max(0)).max().unwrap_or(0);
        let terrain_max = g.iter().flat_map(|&q| runs[q].terrain.iter().copied()).max().unwrap_or(0);
        let terrain_min = g.iter().flat_map(|&q| runs[q].terrain.iter().copied()).min().unwrap_or(terrain_max);
        let total_length: usize = g.iter().map(|&q| runs[q].cell.len() - 1).sum();

        // ramps: a way at a free end, raised and no bridge, carries the deck down
        let mut ramp_ends: HashSet<&End> = HashSet::new();
        let mut pending: Vec<(String, Vec<[f64; 2]>, End, i32)> = Vec::new();
        for &q in g {
            for e in &runs[q].ends {
                if !is_boundary(e) {
                    continue;
                }
                for gers in src.way_ends.get(e).into_iter().flatten() {
                    let way = &src.ways[gers];
                    let raised = value_at(way.row.levels.as_deref(), 0.5).is_some_and(|l| l >= 1.0);
                    if way.row.flagged() || !raised || claimed.contains(gers) {
                        continue;
                    }
                    let Some(line) = way_from(way, e) else { continue };
                    let far = line[line.len() - 1];
                    if group_nodes.contains(&End::at(far)) {
                        continue;
                    }
                    let Ok(ground) = sample_cells(&src.ground, &[far]) else { continue };
                    pending.push((gers.clone(), line, e.clone(), ground[0]));
                    claimed.insert(gers.clone());
                    ramp_ends.insert(e);
                }
            }
        }
        let anchors_end = |n: &End| is_boundary(n) && !ramp_ends.contains(n) && !joins_other(n);

        // the profile's axis: the two member ends furthest apart
        let end_points: Vec<[f64; 2]> = g.iter().flat_map(|&q| [runs[q].cell[0], runs[q].cell[runs[q].cell.len() - 1]]).collect();
        let mut axis = (end_points[0], end_points[0]);
        let mut best = 0.0;
        for (i, a) in end_points.iter().enumerate() {
            for b in &end_points[i + 1..] {
                let d = (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
                if d > best {
                    (best, axis) = (d, (*a, *b));
                }
            }
        }
        let axis_len = best.sqrt();
        let dir = if axis_len > 0.0 {
            [(axis.1[0] - axis.0[0]) / axis_len, (axis.1[1] - axis.0[1]) / axis_len]
        } else {
            [0.0, 0.0]
        };
        let us: Vec<Vec<f64>> = g
            .iter()
            .map(|&q| runs[q].cell.iter().map(|c| (c[0] - axis.0[0]) * dir[0] + (c[1] - axis.0[1]) * dir[1]).collect())
            .collect();
        let follows_axis = axis_len >= 1.0 && us.iter().all(|u| is_monotonic(u, f64::from(units.c(PROFILE_MONOTONIC_TOLERANCE_M))));

        // what the deck must meet or clear, as (axis position, height)
        let mut reqs: Vec<(f64, f64)> = Vec::new();
        let mut anchors: Vec<(f64, i32)> = Vec::new();
        let mut free_end_us: Vec<f64> = Vec::new();
        let mut raised_top = terrain_max;
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            let last = r.cell.len() - 1;
            for (e, i) in [(&r.ends[0], 0), (&r.ends[1], last)] {
                if anchors_end(e) {
                    anchors.push((us[m][i], r.terrain[i]));
                    reqs.push((us[m][i], f64::from(r.terrain[i])));
                } else if let Some(&y) = joints.get(e) {
                    anchors.push((us[m][i], y));
                    reqs.push((us[m][i], f64::from(y)));
                    raised_top = raised_top.max(y);
                } else if is_boundary(e) {
                    free_end_us.push(us[m][i]);
                }
            }
        }
        // a clearance is capped so the deck can still come down to every anchor
        let reachable = |u: f64| anchors.iter().map(|&(ua, ya)| f64::from(ya) + (u - ua).abs() * MAX_CLEARANCE_SLOPE).fold(f64::INFINITY, f64::min);
        let mut level_deck = terrain_max;
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            let layer = effective_layer(r);
            for (i, &c) in r.cell.iter().enumerate() {
                let u = us[m][i];
                let ground = r.terrain[i];
                reqs.push((u, f64::from(ground)));
                let mut need = ground;
                let (p, n) = (r.cell[i.saturating_sub(1)], r.cell[(i + 1).min(r.cell.len() - 1)]);
                for o in obstacles.at(c) {
                    // a way meeting the structure runs at its level, not under it
                    if dist_to_segment(c, o.a, o.b) <= o.reach && crosses([n[0] - p[0], n[1] - p[1]], o) && !o.nodes.iter().any(|n| group_nodes.contains(n)) {
                        need = need.max(ground + o.headroom);
                    }
                }
                if let Some(&(lower, lower_layer)) = decks.get(&cell_key(c)) {
                    if lower_layer < layer {
                        need = need.max(lower + units.c(STACKED_DECK_HEADROOM_M));
                    }
                }
                if need > ground {
                    level_deck = level_deck.max(need);
                    let capped = f64::from(need).min(reachable(u)).max(f64::from(ground));
                    reqs.push((u, capped));
                    raised_top = raised_top.max(round(capped) as i32);
                }
            }
        }
        // a long flat span rises to its layer
        let has_boundary_end = g.iter().any(|&q| runs[q].ends.iter().any(&is_boundary));
        let flat_span = has_boundary_end && terrain_max - terrain_min < units.c(FLAT_TERRAIN_DIP_THRESHOLD_M) && total_length >= units.c(SHORT_BRIDGE_LENGTH_M) as usize;
        let clearance = if flat_span { max_layer as i32 * units.c(LAYER_HEIGHT_STEP_M) } else { 0 };
        if clearance > 0 {
            let plateau = terrain_max + clearance;
            level_deck = level_deck.max(plateau);
            let (u_min, u_max) = us.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), &u| (a.min(u), b.max(u)));
            let span = (u_max - u_min).max(0.0);
            let ramp = ramp_len(span, units).min(span / 2.0);
            let mut placed = false;
            for &u in us.iter().flatten() {
                if anchors.iter().all(|a| (u - a.0).abs() >= ramp) {
                    reqs.push((u, f64::from(plateau)));
                    placed = true;
                }
            }
            if !placed {
                reqs.push(((u_min + u_max) / 2.0, f64::from(plateau)));
            }
            raised_top = raised_top.max(plateau);
        }
        for &u in &free_end_us {
            reqs.push((u, f64::from(raised_top)));
        }
        let hull = if follows_axis { upper_hull(reqs) } else { Vec::new() };
        let mut ys: Vec<Vec<i32>> = us
            .iter()
            .map(|row| row.iter().map(|&u| if follows_axis { round(hull_at(&hull, u)) as i32 } else { level_deck }).collect())
            .collect();

        // an end above the ground ramps down to it inside the member
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            let total = ys[m].len();
            let (gs, ge) = (r.terrain[0], r.terrain[total - 1]);
            let ramp_s = anchors_end(&r.ends[0]) && ys[m][0] > gs + 1;
            let ramp_e = anchors_end(&r.ends[1]) && ys[m][total - 1] > ge + 1;
            let cap = if ramp_s && ramp_e { (total / 2).max(1) } else { total - 1 };
            let len = (ramp_len(total as f64, units) as usize).clamp(1, cap);
            if ramp_s {
                let top = ys[m][len];
                for (k, y) in ys[m][..len].iter_mut().enumerate() {
                    *y = (*y).min(round(f64::from(gs) + f64::from(top - gs) * k as f64 / len as f64) as i32);
                }
            }
            if ramp_e {
                let top = ys[m][total - 1 - len];
                for k in 0..len {
                    let y = &mut ys[m][total - 1 - k];
                    *y = (*y).min(round(f64::from(ge) + f64::from(top - ge) * k as f64 / len as f64) as i32);
                }
            }
            for (y, &t) in ys[m].iter_mut().zip(&r.terrain) {
                *y = (*y).max(t);
            }
        }

        // a ramp runs from the deck's end down to the ground at its far end
        for (gers, line, e, ground) in pending {
            let deck_y = g
                .iter()
                .enumerate()
                .find_map(|(m, &q)| {
                    if runs[q].ends[0] == e {
                        ys[m].first().copied()
                    } else if runs[q].ends[1] == e {
                        ys[m].last().copied()
                    } else {
                        None
                    }
                })
                .unwrap_or(level_deck);
            let way = &src.ways[&gers];
            let cells = cells_along(&line);
            let denom = (cells.len() - 1).max(1) as f64;
            lines.push(graded(
                way,
                cells
                    .iter()
                    .enumerate()
                    .map(|(k, p)| [p[0], p[1], round(f64::from(deck_y) + f64::from(ground - deck_y) * k as f64 / denom)])
                    .collect(),
                units,
            ));
            report.ramps += 1;
        }

        // what the structures after it meet and clear
        for (m, &q) in g.iter().enumerate() {
            let r = &runs[q];
            for (e, y) in [(&r.ends[0], ys[m][0]), (&r.ends[1], ys[m][ys[m].len() - 1])] {
                let j = joints.entry(e.clone()).or_insert(y);
                *j = (*j).max(y);
            }
            let layer = effective_layer(r);
            let half = i64::from(r.half);
            for (&c, &y) in r.cell.iter().zip(&ys[m]) {
                let (x, z) = cell_key(c);
                for dx in -half..=half {
                    for dz in -half..=half {
                        let e = decks.entry((x + dx, z + dz)).or_insert((y, layer));
                        if y > e.0 {
                            *e = (y, layer);
                        }
                    }
                }
            }
            out.push((q, std::mem::take(&mut ys[m])));
        }
    }
    Bridges { ys: out, decks, ramp_ways: claimed }
}

// ---- rail bridges: on a road deck, or one level a viaduct with ramps at its free ends ----

fn rail_bridges(runs: &[Run], obstacles: &Obstacles, decks: &Decks, units: Units, report: &mut Report) -> Vec<(usize, Vec<i32>)> {
    let rail: Vec<usize> = (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Deck && runs[q].rail()).collect();
    let mut out = Vec::new();
    let mut ends_count: HashMap<&End, usize> = HashMap::new();
    for &q in &rail {
        for e in &runs[q].ends {
            *ends_count.entry(e).or_default() += 1;
        }
    }
    // a track along a road deck of its own layer rides it, nearly all of it or not at all
    let reach = i64::from(units.c(RAIL_ON_DECK_REACH_M));
    let mut level: Vec<usize> = Vec::new();
    for &q in &rail {
        let r = &runs[q];
        let layer = effective_layer(r);
        let on: Vec<Option<i32>> = r
            .cell
            .iter()
            .map(|&c| {
                let (x, z) = cell_key(c);
                let mut best: Option<(i64, i32)> = None;
                for dx in -reach..=reach {
                    for dz in -reach..=reach {
                        if let Some(&(y, l)) = decks.get(&(x + dx, z + dz)) {
                            let d = dx.abs() + dz.abs();
                            if l == layer && best.is_none_or(|(bd, _)| d < bd) {
                                best = Some((d, y));
                            }
                        }
                    }
                }
                best.map(|b| b.1)
            })
            .collect();
        if on.iter().filter(|y| y.is_some()).count() * 10 < on.len() * 9 {
            level.push(q);
            continue;
        }
        let mut ys = on;
        for i in 1..ys.len() {
            if ys[i].is_none() {
                ys[i] = ys[i - 1];
            }
        }
        for i in (0..ys.len().saturating_sub(1)).rev() {
            if ys[i].is_none() {
                ys[i] = ys[i + 1];
            }
        }
        out.push((q, ys.into_iter().zip(&r.terrain).map(|(y, &t)| y.unwrap_or_default().max(t)).collect()));
    }
    let mut uf = Uf((0..level.len()).collect());
    let mut at_end: HashMap<(&End, i64), usize> = HashMap::new();
    for (i, &q) in level.iter().enumerate() {
        for e in &runs[q].ends {
            match at_end.get(&(e, effective_layer(&runs[q]))) {
                Some(&j) => uf.union(i, j),
                None => {
                    at_end.insert((e, effective_layer(&runs[q])), i);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &l) in level.iter().enumerate() {
        groups.entry(uf.find(i)).or_default().push(l);
    }
    for g in groups.into_values() {
        report.rail_bridges += 1;
        let (mut tmax, mut tmin, mut floor, mut layer) = (i32::MIN, i32::MAX, i32::MIN, 0);
        for &q in &g {
            let r = &runs[q];
            layer = layer.max(effective_layer(r));
            for (j, &c) in r.cell.iter().enumerate() {
                let ground = r.terrain[j];
                (tmax, tmin) = (tmax.max(ground), tmin.min(ground));
                let (p, n) = (r.cell[j.saturating_sub(1)], r.cell[(j + 1).min(r.cell.len() - 1)]);
                for o in obstacles.at(c) {
                    if dist_to_segment(c, o.a, o.b) <= o.reach && crosses([n[0] - p[0], n[1] - p[1]], o) && !o.nodes.iter().any(|n| r.nodes.contains(n)) {
                        floor = floor.max(ground + o.headroom);
                    }
                }
                if let Some(&(lower, lower_layer)) = decks.get(&cell_key(c)) {
                    if lower_layer < effective_layer(r) {
                        floor = floor.max(lower + units.c(STACKED_DECK_HEADROOM_M));
                    }
                }
            }
        }
        // a flat span lifts clear, a level more a layer; a valley span sits on its banks
        let deck = if tmax - tmin < units.c(RAIL_BRIDGE_DIP_THRESHOLD_M) {
            tmax + units.c(RAIL_BRIDGE_FLAT_CLEARANCE_M) + (layer as i32 - 1).max(0) * units.c(LAYER_HEIGHT_STEP_M)
        } else {
            tmax
        }
        .max(floor);
        // each way ramps from its free ends, no steeper than a cell a cell (Arnis `generate_rail_bridge`)
        for &q in &g {
            let r = &runs[q];
            let total = r.cell.len();
            let last = total - 1;
            let internal = [ends_count[&r.ends[0]] > 1, ends_count[&r.ends[1]] > 1];
            let (gs, ge) = (r.terrain[0], r.terrain[last]);
            let mut needed = 0;
            if !internal[0] {
                needed = needed.max((deck - gs).max(0) as usize);
            }
            if !internal[1] {
                needed = needed.max((deck - ge).max(0) as usize);
            }
            let ramp = ((total as f64 * RAIL_BRIDGE_RAMP_FRACTION) as usize)
                .clamp(units.c(RAIL_BRIDGE_RAMP_M[0]) as usize, units.c(RAIL_BRIDGE_RAMP_M[1]) as usize)
                .max(needed + 1);
            let denom = ramp.saturating_sub(1).max(1) as f64;
            let ys = (0..total)
                .map(|k| {
                    let s = if internal[0] {
                        deck
                    } else {
                        round(f64::from(gs) + f64::from(deck - gs) * (k as f64 / denom).min(1.0)) as i32
                    };
                    let e = if internal[1] {
                        deck
                    } else {
                        round(f64::from(ge) + f64::from(deck - ge) * ((last - k) as f64 / denom).min(1.0)) as i32
                    };
                    s.min(e).max(r.terrain[k])
                })
                .collect();
            out.push((q, ys));
        }
    }
    out
}

// ---- road tunnels: approaches sink into the portals, the bore as deep as its cover asks ----

/// `drop` cells under grade at cell `anchor`, easing back over `run` cells a cell.
#[derive(Clone, Copy)]
struct Claim {
    anchor: i32,
    drop: i32,
    run: i32,
}

fn road_tunnels(src: &Source, runs: &[Run], ramp_ways: &HashSet<String>, units: Units, lines: &mut Vec<(Line, Drawn)>, report: &mut Report) -> Vec<(usize, Vec<i32>, Vec<bool>)> {
    let tunnels: Vec<usize> = (0..runs.len()).filter(|&q| runs[q].piece.form == Form::Bore && !runs[q].rail()).collect();
    // a node two tunnels share, or an end the ring cut, stays at depth
    let mut owner: HashMap<&End, Id> = HashMap::new();
    let mut shared: HashSet<&End> = HashSet::new();
    for &q in &tunnels {
        for n in &runs[q].nodes {
            let id = runs[q].piece.id();
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
    let internal = |q: usize, e: usize| runs[q].clipped[e] || shared.contains(&runs[q].ends[e]);
    let layer_extra = |r: &Run| (-(r.piece.layer.unwrap_or(0) + 1)).clamp(0, TUNNEL_MAX_LAYERS_BELOW) as i32 * units.c(TUNNEL_LAYER_DROP_M);

    // ---- the approaches: ways at grade from a portal, as far as the drop needs and the road goes on;
    // a portal on a bridge's middle has none, so it stays at the terrain under the deck, as in Arnis ----
    let portal_nodes: HashSet<&End> = tunnels.iter().flat_map(|&q| (0..2).filter(move |&e| !internal(q, e)).map(move |e| &runs[q].ends[e])).collect();
    let surface = |gers: &str| src.ways.get(gers).filter(|w| !w.row.flagged() && w.row.subtype == "road" && !ramp_ways.contains(gers));
    let ways_len = |gers: &str| cells_along(&src.ways[gers].line.points).len() as i32;
    let mut claims: BTreeMap<String, Vec<Claim>> = BTreeMap::new();
    let mut drops: HashMap<End, i32> = HashMap::new();
    for &q in &tunnels {
        let r = &runs[q];
        let want = units.c(TUNNEL_COVER_DROP_M) + layer_extra(r);
        let walking = is_non_vehicular(r.piece.class.as_deref().unwrap_or(""));
        for e in 0..2 {
            let node = &r.ends[e];
            if internal(q, e) || drops.contains_key(node) || want <= 0 {
                continue;
            }
            let budget = want * TUNNEL_RAMP_RUN;
            let mut taken: HashSet<String> = HashSet::new();
            let mut branches: Vec<Vec<(String, bool, i32)>> = Vec::new();
            let eligible = |at: &End, taken: &HashSet<String>, claims: &BTreeMap<String, Vec<Claim>>| -> Vec<String> {
                let mut out: Vec<String> = src
                    .way_ends
                    .get(at)
                    .into_iter()
                    .flatten()
                    .filter(|g| !taken.contains(*g) && claims.get(*g).is_none_or(|c| c.len() < 2))
                    .filter(|g| surface(g).is_some_and(|w| is_non_vehicular(w.row.class.as_deref().unwrap_or("")) == walking))
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
                let (mut gers, mut at, mut dist) = (first, node.clone(), 0);
                loop {
                    let w = &src.ways[&gers];
                    let at_start = End::at(w.line.points[0]) == at;
                    chain.push((gers.clone(), at_start, dist));
                    taken.insert(gers.clone());
                    dist += ways_len(&gers) - 1;
                    at = End::at(if at_start { w.line.points[w.line.points.len() - 1] } else { w.line.points[0] });
                    if dist >= budget || chain.len() >= TUNNEL_APPROACH_WAYS || portal_nodes.contains(&at) {
                        break;
                    }
                    // only where the road goes on: a junction stays at grade
                    let next = eligible(&at, &taken, &claims);
                    if next.len() != 1 {
                        break;
                    }
                    gers = next[0].clone();
                }
                branches.push(chain);
            }
            // every branch takes the same descent, or one ends in a cliff
            let Some(avail) = branches.iter().filter_map(|b| b.last().map(|(g, _, d0)| d0 + ways_len(g) - 1)).min() else {
                continue;
            };
            let drop = want.min(avail);
            if drop <= 0 {
                continue;
            }
            let run = (avail / drop).clamp(1, TUNNEL_RAMP_RUN);
            for (gers, at_start, d0) in branches.into_iter().flatten() {
                let last = ways_len(&gers) - 1;
                let c = claims.entry(gers).or_default();
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
    for (gers, cs) in &claims {
        let w = &src.ways[gers];
        let cells = cells_along(&w.line.points);
        let Ok(terrain) = sample_cells(&src.ground, &cells) else { continue };
        let points = cells
            .iter()
            .zip(&terrain)
            .enumerate()
            .map(|(k, (p, &t))| {
                let drop = cs.iter().map(|c| c.drop - (k as i32 - c.anchor).abs() / c.run).max().unwrap_or(0).max(0);
                [p[0], p[1], f64::from(t - drop)]
            })
            .collect();
        lines.push(graded(w, points, units));
        report.approaches += 1;
    }

    // ---- each bore (Arnis `generate_highway_tunnel_shell`); the world has no bedrock to stop it ----
    let roof = units.c(TUNNEL_ROOF_COVER_M);
    let mut out = Vec::new();
    for &q in &tunnels {
        report.tunnels += 1;
        let r = &runs[q];
        let n = r.cell.len();
        let last = n - 1;
        let wall = r.half + 1;
        // the lowest ground over the bore's footprint decides its roof
        let cover: Vec<i32> =
            r.at.iter()
                .zip(&r.terrain)
                .map(|(p, &t)| {
                    let around: Vec<[f64; 2]> = (-wall..=wall)
                        .flat_map(|dx| (-wall..=wall).map(move |dz| [p[0] + to_db(f64::from(dx) * U_PER_CELL), p[1] + to_db(f64::from(dz) * U_PER_CELL)]))
                        .filter(|q| src.loaded.contains(&tile_of(*q)))
                        .collect();
                    sample_cells(&src.ground, &around).map_or(t, |v| v.into_iter().fold(t, i32::min))
                })
                .collect();
        let (gs, ge) = (r.terrain[0], r.terrain[last]);
        let (internal_s, internal_e) = (internal(q, 0), internal(q, 1));
        let drop_cover = units.c(TUNNEL_COVER_DROP_M) + layer_extra(r);
        let start_drop = if internal_s { 0 } else { drops.get(&r.ends[0]).copied().unwrap_or(0) };
        let end_drop = if internal_e { 0 } else { drops.get(&r.ends[1]).copied().unwrap_or(0) };
        let denom = last.max(1) as f64;
        // as deep as cover asks, ramping down from whatever the portals reached
        let mut ys: Vec<i32> = (0..n)
            .map(|i| {
                let grade = round(f64::from(gs) + f64::from(ge - gs) * i as f64 / denom) as i32;
                let desired = grade.min(cover[i] - drop_cover);
                let ramp_s = if internal_s { i32::MIN } else { gs - start_drop - i as i32 / TUNNEL_RAMP_RUN };
                let ramp_e = if internal_e { i32::MIN } else { ge - end_drop - (last - i) as i32 / TUNNEL_RAMP_RUN };
                desired.max(ramp_s.max(ramp_e)).min(r.terrain[i])
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
        // one valley, not several
        if let Some(m) = (0..n).min_by_key(|&i| ys[i]) {
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

/// Stretches of cells alike, each reaching to the next's first cell so the
/// line goes on unbroken, with whether it is open.
fn split_by(covered: &[bool]) -> Vec<([usize; 2], bool)> {
    let mut out = Vec::new();
    let mut from = 0;
    for i in 1..=covered.len() {
        if i == covered.len() || covered[i] != covered[from] {
            out.push(([from, i.min(covered.len() - 1)], !covered[from]));
            from = i;
        }
    }
    out
}

/// Cells `a..=b` of a run as a line at heights `ys`: the piece's vertices and
/// where the slope bends, no more.
fn part_of(r: &Run, ys: &[i32], [a, b]: [usize; 2], clear: u8) -> (Line, Drawn) {
    let step = to_db(U_PER_CELL);
    let along = |k: usize| ((r.k0 + k) as f64 * step).min(r.piece.length);
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
    for seg in r.piece.points.windows(2) {
        distance += libm::hypot(seg[1][0] - seg[0][0], seg[1][1] - seg[0][1]);
        if distance > along(a) && distance < along(b) {
            let i = ((distance / step).floor() as usize).saturating_sub(r.k0).clamp(a, b - 1);
            let z = f64::from(ys[i]) + f64::from(ys[i + 1] - ys[i]) * (distance - along(i)) / step;
            keep.push((distance, [seg[1][0], seg[1][1]], round(z)));
        }
    }
    keep.sort_by(|x, y| x.0.total_cmp(&y.0));
    keep.dedup_by(|x, y| x.0 == y.0);
    let fraction = |d: f64| (r.piece.start + d) / r.piece.way_length;
    (
        Line {
            subtype: r.piece.subtype.clone(),
            class: r.piece.class.clone(),
            width_cells: r.width_cells,
            form: r.piece.form,
            points: keep.iter().map(|(_, p, z)| [p[0], p[1], *z]).collect(),
            clear,
        },
        Drawn {
            gers_id: r.piece.gers_id.clone(),
            range: [fraction(along(a)), fraction(along(b))],
        },
    )
}

#[cfg(test)]
#[path = "arnis.test.rs"]
mod tests;
