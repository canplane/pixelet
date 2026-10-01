// pxl-geo-bake/deck/structure.rs

//! A structure: the bridge pieces that meet end to end, solved as one —
//! a viaduct split into many segments, an interchange's ramps — and the
//! graph of points along them its profile is solved on.
//!
//! Pieces meet where an end of one is an end of another. A level is not
//! asked: a continuous viaduct's tagging changes its layer along it, and
//! where two decks of one structure cross, the lower is cleared like any
//! other (deck.rs).

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

/// The pieces, by index, of each structure.
pub fn group(pieces: &[Piece]) -> Vec<Vec<usize>> {
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
/// end, an end its pieces share being one point.
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
    pub fn build(pieces: &[Piece], members: &[usize]) -> Graph {
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
