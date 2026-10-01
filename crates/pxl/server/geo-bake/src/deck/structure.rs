// pxl-geo-bake/deck/structure.rs

//! A structure: the bridge pieces that meet end to end, solved as one —
//! a viaduct split into many segments, an interchange's ramps — and the
//! graph of points along them its profile is solved on.
//!
//! Pieces meet where an end of one is an end of another, and where two run
//! side by side — the two carriageways of a road drawn as two, a sidewalk
//! beside its road — which one bridge carries at one height: their ends lie
//! within a margin of each other's edge and they head the same way, as
//! Arnis unions its parallel decks. A level is not asked: a continuous
//! viaduct's tagging changes its layer along it, and where two decks of one
//! structure cross, the lower is cleared like any other (deck.rs).

use std::collections::HashMap;

use crate::deck::clearance::STEP;
use crate::deck::piece::Piece;
use crate::terrain::pipeline::overture::between::Line;

/// An end's key: its point to the centimetre, which two segments that share
/// a connector agree on.
type Key = (i64, i64);

fn key_of(p: [f64; 3]) -> Key {
    ((p[0] * 100.0).round() as i64, (p[1] * 100.0).round() as i64)
}

/// How far apart two decks' edges may be and still be one bridge's, in
/// metres: Arnis's.
const SIDE_GAP: f64 = 2.0;

/// How far apart two decks' headings may be and still run side by side.
const SIDE_HEADING: f64 = 20.0 * core::f64::consts::PI / 180.0;

/// Two pieces' ends that lie side by side: each as its piece and whether it
/// is the piece's last end, and how far apart they are.
#[derive(Clone, Copy)]
pub struct Beside {
    pub a: (usize, bool),
    pub b: (usize, bool),
    pub d: f64,
}

/// An end's point and the way the piece heads into itself from it.
fn end_of(p: &Piece, last: bool) -> ([f64; 2], [f64; 2]) {
    let n = p.points.len();
    let (at, next) = if last { (p.points[n - 1], p.points[n - 2]) } else { (p.points[0], p.points[1]) };
    let (dx, dy) = (next[0] - at[0], next[1] - at[1]);
    let len = dx.hypot(dy).max(f64::EPSILON);
    ([at[0], at[1]], [dx / len, dy / len])
}

/// Every pair of ends of different pieces that lie side by side.
pub fn side_by_side(pieces: &[Piece]) -> Vec<Beside> {
    let widest = pieces.iter().map(|p| p.width_m).fold(0.0, f64::max);
    let cell = widest + SIDE_GAP;
    let mut grid: HashMap<(i64, i64), Vec<(usize, bool)>> = HashMap::new();
    let cell_of = |at: [f64; 2]| ((at[0] / cell).floor() as i64, (at[1] / cell).floor() as i64);
    for (i, p) in pieces.iter().enumerate() {
        for last in [false, true] {
            grid.entry(cell_of(end_of(p, last).0)).or_default().push((i, last));
        }
    }
    let mut out = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        for last in [false, true] {
            let (at, heading) = end_of(p, last);
            let (cx, cy) = cell_of(at);
            for (j, other_last) in (cx - 1..=cx + 1).flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y))).filter_map(|c| grid.get(&c)).flatten().copied() {
                if j <= i {
                    continue;
                }
                let (other, other_heading) = end_of(&pieces[j], other_last);
                let d = (at[0] - other[0]).hypot(at[1] - other[1]);
                let meet = key_of([at[0], at[1], 0.0]) == key_of([other[0], other[1], 0.0]);
                let near = d <= (p.width_m + pieces[j].width_m) / 2.0 + SIDE_GAP;
                let parallel = (heading[0] * other_heading[0] + heading[1] * other_heading[1]).clamp(-1.0, 1.0).acos() <= SIDE_HEADING;
                if !meet && near && parallel {
                    out.push(Beside { a: (i, last), b: (j, other_last), d });
                }
            }
        }
    }
    out
}

/// The pieces, by index, of each structure.
pub fn group(pieces: &[Piece], beside: &[Beside]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..pieces.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut first_at: HashMap<Key, usize> = HashMap::new();
    for (i, p) in pieces.iter().enumerate() {
        for end in [p.points[0], *p.points.last().unwrap()] {
            match first_at.get(&key_of(end)) {
                Some(&j) => {
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    parent[a] = b;
                }
                None => {
                    first_at.insert(key_of(end), i);
                }
            }
        }
    }
    for b in beside {
        let (x, y) = (root(&mut parent, b.a.0), root(&mut parent, b.b.0));
        parent[x] = y;
    }
    let mut by_root: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..pieces.len() {
        let r = root(&mut parent, i);
        by_root.entry(r).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = by_root.into_values().collect();
    out.sort_by_key(|s| s[0]);
    out
}

/// A structure's points, every `STEP` metres along each piece and at every
/// end, an end its pieces share being one point and two ends side by side
/// joined across.
pub struct Graph {
    /// Each point, in EPSG:3857 metres.
    pub at: Vec<[f64; 2]>,
    /// Each point's neighbours along a piece, and how far.
    pub adj: Vec<Vec<(u32, f64)>>,
    /// Each piece's points in order, with how far along the piece each is.
    pub along: Vec<Vec<(u32, f64)>>,
    /// The points at an end only one piece reaches: where the structure
    /// comes down to the ground.
    pub free_ends: Vec<u32>,
}

impl Graph {
    pub fn build(pieces: &[Piece], members: &[usize], beside: &[Beside]) -> Graph {
        let mut g = Graph {
            at: Vec::new(),
            adj: Vec::new(),
            along: Vec::new(),
            free_ends: Vec::new(),
        };
        let mut ends: HashMap<Key, (u32, u32)> = HashMap::new();
        let point = |g: &mut Graph, p: [f64; 3]| -> u32 {
            g.at.push([p[0], p[1]]);
            g.adj.push(Vec::new());
            (g.at.len() - 1) as u32
        };
        for &m in members {
            let Some(line) = Line::of(&pieces[m].points) else {
                g.along.push(Vec::new());
                continue;
            };
            let len = line.length();
            let steps = (len / STEP).ceil().max(1.0) as usize;
            let mut nodes: Vec<(u32, f64)> = Vec::with_capacity(steps + 1);
            for k in 0..=steps {
                let d = len * k as f64 / steps as f64;
                let p = line.at(d);
                let node = if k == 0 || k == steps {
                    let e = ends.entry(key_of(p)).or_insert_with(|| (u32::MAX, 0));
                    if e.0 == u32::MAX {
                        e.0 = point(&mut g, p);
                    }
                    e.1 += 1;
                    e.0
                } else {
                    point(&mut g, p)
                };
                if let Some(&(prev, pd)) = nodes.last() {
                    g.adj[prev as usize].push((node, d - pd));
                    g.adj[node as usize].push((prev, d - pd));
                }
                nodes.push((node, d));
            }
            g.along.push(nodes);
        }
        let member: HashMap<usize, usize> = members.iter().enumerate().map(|(k, &m)| (m, k)).collect();
        let end_node = |g: &Graph, (piece, last): (usize, bool)| -> Option<u32> {
            let nodes = &g.along[*member.get(&piece)?];
            if last { nodes.last() } else { nodes.first() }.map(|&(n, _)| n)
        };
        for b in beside {
            if let (Some(x), Some(y)) = (end_node(&g, b.a), end_node(&g, b.b)) {
                g.adj[x as usize].push((y, b.d));
                g.adj[y as usize].push((x, b.d));
            }
        }
        g.free_ends = ends.into_values().filter(|&(_, n)| n == 1).map(|(node, _)| node).collect();
        g.free_ends.sort_unstable();
        g
    }

    /// The point of piece `k` (its place among the structure's members)
    /// nearest `d` metres along it.
    pub fn nearest(&self, k: usize, d: f64) -> Option<u32> {
        let nodes = &self.along[k];
        let i = nodes.partition_point(|&(_, nd)| nd < d);
        [i.checked_sub(1), Some(i)].into_iter().flatten().filter_map(|i| nodes.get(i)).min_by(|a, b| (a.1 - d).abs().total_cmp(&(b.1 - d).abs())).map(|&(n, _)| n)
    }
}
