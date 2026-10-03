// pxl-geo-bake/deck/profile.rs

//! A structure's surface in z. A deck's points have a floor, a bore's a
//! ceiling. It runs on the line between its ends, held at their bounds, and
//! bends only where that line breaks a bound, the bend coming away at
//! `PREFERRED_SLOPE`, steeper only to land on a point's own by `halo` or to
//! pass every bound the other way within `halo`.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::deck::param::PREFERRED_SLOPE;

pub struct Problem<'a> {
    pub adj: &'a [Vec<(u32, f64)>],
    pub ground: &'a [f64],
    /// −∞ where none.
    pub floor: &'a [f64],
    /// +∞ where none.
    pub ceiling: &'a [f64],
    /// The form's own bound at each point, before what passes under or over
    /// it: what a bend comes away from, and the surface of a structure with no end.
    pub base: &'a [f64],
    /// A structure's end, at its bound there.
    pub ends: &'a [bool],
    /// How far, in metres, a point's bound reaches: its own latitude's, so no
    /// window's choice of points moves it.
    pub halo: &'a [f64],
}

/// The surface at each point.
pub fn solve(p: &Problem) -> Vec<f64> {
    let n = p.ground.len();
    let neg = |v: &[f64]| v.iter().map(|x| -x).collect::<Vec<f64>>();
    let line = line(p);
    let lower = envelope(p, p.floor, p.ceiling, 1.0, &line);
    let upper = neg(&envelope(p, &neg(p.ceiling), &neg(p.floor), -1.0, &line));
    (0..n).map(|v| lower[v].max(upper[v].min(line[v]))).collect()
}

/// Between a structure's ends by inverse distance along it: on a chain, the
/// straight line between its two, each at its bound; with none, a ring, the
/// form's own bound.
fn line(p: &Problem) -> Vec<f64> {
    let n = p.ground.len();
    let (mut sum, mut weight) = (vec![0.0; n], vec![0.0; n]);
    let mut at_end: Vec<Option<f64>> = vec![None; n];
    let mut near = Near::new(n);
    for e in (0..n).filter(|&e| p.ends[e]) {
        let z = if p.floor[e].is_finite() { p.floor[e] } else { p.ceiling[e] };
        at_end[e] = Some(z);
        near.from(p.adj, e as u32, f64::INFINITY);
        for &v in near.reached.iter().filter(|&&v| near.dist[v as usize] > 0.0) {
            let w = 1.0 / near.dist[v as usize];
            (sum[v as usize], weight[v as usize]) = (sum[v as usize] + z * w, weight[v as usize] + w);
        }
    }
    (0..n).map(|v| at_end[v].unwrap_or(if weight[v] > 0.0 { sum[v] / weight[v] } else { p.base[v] })).collect()
}

/// In `q = sign · z`: what the bounds `least` hold up, each coming away no
/// steeper than it must to pass the bounds `most`.
fn envelope(p: &Problem, least: &[f64], most: &[f64], sign: f64, line: &[f64]) -> Vec<f64> {
    let n = least.len();
    let own = |v: usize| if least[v].is_finite() { least[v] } else { sign * p.ground[v] };
    let mut env = least.to_vec();
    let mut near = Near::new(n);
    for u in (0..n).filter(|&u| least[u].is_finite()) {
        near.from(p.adj, u as u32, p.halo[u]);
        let reached = || near.reached.iter().map(|&v| (v as usize, near.dist[v as usize]));
        let pass = reached().filter(|&(w, d)| most[w].is_finite() && d > 0.0).map(|(w, d)| (least[u] - most[w]) / d).fold(PREFERRED_SLOPE, f64::max);
        for (v, d) in reached() {
            // steeper only to land on v's own by the halo, so no step there
            let slope = pass.max((least[u] - own(v)) / p.halo[u]);
            // a structure bends from the line between its ends by that grade
            env[v] = env[v].max(sign * (line[v] - line[u]) + least[u] - slope * d);
        }
    }
    env
}

/// Distances from a point within a reach; resets only what it reached.
struct Near {
    dist: Vec<f64>,
    reached: Vec<u32>,
}

impl Near {
    fn new(n: usize) -> Near {
        Near {
            dist: vec![f64::INFINITY; n],
            reached: Vec::new(),
        }
    }

    fn from(&mut self, adj: &[Vec<(u32, f64)>], a: u32, reach: f64) {
        for &v in &self.reached {
            self.dist[v as usize] = f64::INFINITY;
        }
        self.reached.clear();
        let mut heap = BinaryHeap::new();
        self.dist[a as usize] = 0.0;
        self.reached.push(a);
        heap.push(Entry {
            value: 0.0,
            node: a,
            best: Ordering::Less,
        });
        while let Some(Entry { value, node, .. }) = heap.pop() {
            if self.dist[node as usize] != value {
                continue;
            }
            for &(next, d) in &adj[node as usize] {
                let y = value + d;
                if y <= reach && y < self.dist[next as usize] {
                    if self.dist[next as usize] == f64::INFINITY {
                        self.reached.push(next);
                    }
                    self.dist[next as usize] = y;
                    heap.push(Entry {
                        value: y,
                        node: next,
                        best: Ordering::Less,
                    });
                }
            }
        }
    }
}

struct Entry {
    value: f64,
    node: u32,
    best: Ordering,
}

impl PartialEq for Entry {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Entry {
    fn cmp(&self, o: &Self) -> Ordering {
        let c = self.value.total_cmp(&o.value).then(o.node.cmp(&self.node));
        if self.best == Ordering::Less {
            c.reverse()
        } else {
            c
        }
    }
}

#[cfg(test)]
#[path = "profile.test.rs"]
mod test;
