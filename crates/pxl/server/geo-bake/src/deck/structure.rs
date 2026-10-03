// pxl-geo-bake/deck/structure.rs

//! A structure: pieces that meet end to end, or run side by side within a
//! gap and heading (as Arnis unions parallel decks), solved as one. Level is
//! not compared: a viaduct's layer changes along it.

use pxl_spec::space::adapter::epsg3857::{from_db, scale_at};
use std::collections::HashMap;

use crate::deck::param::{SIDE_GAP_M, STEP_M};
use crate::deck::piece::{End, Piece};
use crate::terrain::pipeline::corridor::SIDE_HEADING_COS;
use crate::terrain::pipeline::overture::between::Line;

#[derive(Clone, Copy)]
pub struct Beside {
    pub a: (usize, bool),
    pub b: (usize, bool),
    pub d: f64,
}

pub fn end_of(p: &Piece, last: bool) -> ([f64; 2], [f64; 2]) {
    let n = p.points.len();
    let (at, next) = if last { (p.points[n - 1], p.points[n - 2]) } else { (p.points[0], p.points[1]) };
    let (dx, dy) = (next[0] - at[0], next[1] - at[1]);
    let len = libm::hypot(dx, dy).max(f64::EPSILON);
    ([at[0], at[1]], [dx / len, dy / len])
}

pub fn side_by_side(pieces: &[Piece]) -> Vec<Beside> {
    let widest = pieces.iter().map(|p| p.width_m).fold(0.0, f64::max);
    let largest = pieces.iter().map(|p| scale_at(from_db(p.points[0][1]))).fold(0.0, f64::max);
    let cell = (widest + SIDE_GAP_M * 2.0) * largest;
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
                let d = libm::hypot(at[0] - other[0], at[1] - other[1]) / scale_at(from_db(at[1]));
                let meet = p.end(last) == pieces[j].end(other_last);
                let near = d <= (p.width_m + pieces[j].width_m) / 2.0 + SIDE_GAP_M;
                let parallel = heading[0] * other_heading[0] + heading[1] * other_heading[1] >= SIDE_HEADING_COS;
                if !meet && near && parallel {
                    out.push(Beside { a: (i, last), b: (j, other_last), d });
                }
            }
        }
    }
    out
}

pub fn group(pieces: &[Piece], beside: &[Beside]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..pieces.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut first_at: HashMap<&End, usize> = HashMap::new();
    for (i, p) in pieces.iter().enumerate() {
        for end in &p.ends {
            match first_at.get(end) {
                Some(&j) => {
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    parent[a] = b;
                }
                None => {
                    first_at.insert(end, i);
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

/// Points every `STEP_M` and at every end, distances in physical metres.
pub struct Graph {
    pub at: Vec<[f64; 2]>,
    pub adj: Vec<Vec<(u32, f64)>>,
    pub along: Vec<Vec<(u32, f64)>>,
    /// Nodes only one member reaches, and what end each is.
    pub free_ends: Vec<(u32, End)>,
}

impl Graph {
    pub fn build(pieces: &[Piece], members: &[usize], beside: &[Beside]) -> Graph {
        let mut g = Graph {
            at: Vec::new(),
            adj: Vec::new(),
            along: Vec::new(),
            free_ends: Vec::new(),
        };
        let mut ends: HashMap<&End, (u32, u32)> = HashMap::new();
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
            // on the way's own lattice, so a piece clipped anywhere samples where the whole does
            let step = STEP_M * len / pieces[m].to_m(len);
            let start = pieces[m].start;
            let first = (libm::floor(start / step) + 1.0) * step - start;
            let inner = (0..).map(|k| first + k as f64 * step).take_while(|&a| a < len);
            let at: Vec<f64> = std::iter::once(0.0).chain(inner).chain(std::iter::once(len)).collect();
            let mut nodes: Vec<(u32, f64)> = Vec::with_capacity(at.len());
            for (k, &along) in at.iter().enumerate() {
                let p = line.at(along);
                let d = pieces[m].to_m(along);
                let node = if k == 0 || k + 1 == at.len() {
                    let e = ends.entry(pieces[m].end(k != 0)).or_insert_with(|| (u32::MAX, 0));
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
        g.free_ends = ends.into_iter().filter(|&(_, (_, n))| n == 1).map(|(end, (node, _))| (node, end.clone())).collect();
        g.free_ends.sort_unstable_by_key(|&(node, _)| node);
        g
    }

    pub fn nearest(&self, k: usize, d: f64) -> Option<u32> {
        let nodes = &self.along[k];
        let i = nodes.partition_point(|&(_, nd)| nd < d);
        [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter_map(|i| nodes.get(i))
            .min_by(|a, b| (a.1 - d).abs().total_cmp(&(b.1 - d).abs()))
            .map(|&(n, _)| n)
    }
}
