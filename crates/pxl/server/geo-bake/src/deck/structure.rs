// pxl-geo-bake/deck/structure.rs

//! A structure: pieces of one form that meet end to end, solved as one.
//! Level is not compared: a viaduct's layer changes along it.

use std::collections::HashMap;

use crate::deck::param::STEP_M;
use crate::deck::piece::{End, Piece};
use crate::terrain::pipeline::overture::between::Line;

pub fn group(pieces: &[Piece]) -> Vec<Vec<usize>> {
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
    pub fn build(pieces: &[Piece], members: &[usize]) -> Graph {
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
