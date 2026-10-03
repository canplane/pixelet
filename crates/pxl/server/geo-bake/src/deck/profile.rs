// pxl-geo-bake/deck/profile.rs

//! A structure's road surface in `q`: a deck's height, a bore's depth. Each
//! point stands at its need, or under another's within `halo` coming down at
//! `PREFERRED_SLOPE`, steeper only where that would not reach its own need
//! within `halo`; so nothing past `halo` moves a point.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::deck::param::PREFERRED_SLOPE;

pub struct Problem<'a> {
    pub adj: &'a [Vec<(u32, f64)>],
    pub need: &'a [f64],
    pub ground: &'a [f64],
    pub carried: &'a [bool],
    /// How far, in metres, a need reaches.
    pub halo: f64,
}

pub struct Profile {
    pub top: Vec<f64>,
    pub forced: Vec<bool>,
}

pub fn solve(p: &Problem) -> Profile {
    let n = p.need.len();
    let adj = carried_adj(p);
    // an approach is its ground: the joint comes down to its next point
    let mut top: Vec<f64> = (0..n).map(|v| if p.carried[v] { p.need[v] } else { p.ground[v] }).collect();
    let mut near = Near::new(n);
    for u in (0..n).filter(|&u| p.carried[u]) {
        near.from(&adj, u as u32, p.halo);
        for &v in &near.reached {
            let (v, d) = (v as usize, near.dist[v as usize]);
            // steeper only to land on v's own need by the halo, so no step there
            let slope = PREFERRED_SLOPE.max((p.need[u] - p.need[v]) / p.halo);
            top[v] = top[v].max(p.need[u] - slope * d);
        }
    }
    let forced = (0..n).map(|v| adj[v].iter().any(|&(w, d)| d > 0.0 && (top[v] - top[w as usize]).abs() / d > PREFERRED_SLOPE + 1e-9)).collect();
    Profile { top, forced }
}

fn carried_adj(p: &Problem) -> Vec<Vec<(u32, f64)>> {
    (0..p.need.len())
        .map(|v| if p.carried[v] { p.adj[v].iter().copied().filter(|&(w, _)| p.carried[w as usize]).collect() } else { Vec::new() })
        .collect()
}

/// Distances from a point within a reach; resets only what it reached.
struct Near {
    dist: Vec<f64>,
    reached: Vec<u32>,
}

impl Near {
    fn new(n: usize) -> Near {
        Near { dist: vec![f64::INFINITY; n], reached: Vec::new() }
    }

    fn from(&mut self, adj: &[Vec<(u32, f64)>], a: u32, reach: f64) {
        for &v in &self.reached {
            self.dist[v as usize] = f64::INFINITY;
        }
        self.reached.clear();
        let mut heap = BinaryHeap::new();
        self.dist[a as usize] = 0.0;
        self.reached.push(a);
        heap.push(Entry { value: 0.0, node: a, best: Ordering::Less });
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
                    heap.push(Entry { value: y, node: next, best: Ordering::Less });
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
