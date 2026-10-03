// pxl-geo-bake/deck/profile.rs

//! A road's surface along a structure and the ways it carries on along, in
//! z. A deck's points have a floor, a bore's a ceiling, and where a way ends
//! on the ground both are the ground. Each bound reaches `halo`, coming away
//! at `PREFERRED_SLOPE`, steeper only to land on a point's own by `halo` or to
//! pass every bound the other way within `halo`. The surface stands on what
//! the floors push up and under what the ceilings push down, at the ground
//! between: so a structure keeps its form to its end, the road leaves it on
//! the way it carries on along, and nothing past twice `halo` moves a point.

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
    /// How far, in metres, a point's bound reaches: its own latitude's, so no
    /// window's choice of points moves it.
    pub halo: &'a [f64],
}

pub struct Profile {
    pub top: Vec<f64>,
    pub forced: Vec<bool>,
}

pub fn solve(p: &Problem) -> Profile {
    let n = p.ground.len();
    let neg = |v: &[f64]| v.iter().map(|x| -x).collect::<Vec<f64>>();
    let lower = envelope(p, p.floor, p.ceiling, 1.0);
    let upper = neg(&envelope(p, &neg(p.ceiling), &neg(p.floor), -1.0));
    let top: Vec<f64> = (0..n).map(|v| lower[v].max(upper[v].min(p.ground[v]))).collect();
    let forced = (0..n).map(|v| p.adj[v].iter().any(|&(w, d)| d > 0.0 && (top[v] - top[w as usize]).abs() / d > PREFERRED_SLOPE + 1e-9)).collect();
    Profile { top, forced }
}

/// In `q = sign · z`: what the bounds `least` hold up, each coming away no
/// steeper than it must to pass the bounds `most`.
fn envelope(p: &Problem, least: &[f64], most: &[f64], sign: f64) -> Vec<f64> {
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
            env[v] = env[v].max(least[u] - slope * d);
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
