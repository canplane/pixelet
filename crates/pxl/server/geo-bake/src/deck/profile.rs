// pxl-geo-bake/deck/profile.rs

//! A structure's deck profile, solved on the graph of its points.
//!
//! Three things decide a deck's top at a point, all along the structure and
//! never across it:
//!
//! - **The line between where it meets the ground.** A bridge is carried
//!   from end to end, so where nothing is under it, it runs straight between
//!   its ends' ground — over a valley as much as over a flat. On a graph that
//!   is the ends' heights weighted by how near each is, which on a single
//!   span is exactly the straight line. Only the carried part has a line: an
//!   approach is a way on the ground, and its ends are where it joins the
//!   bridge, so the line never runs from one approach's far end to the
//!   other's across whatever lies between.
//! - **What it has to clear.** Each point asks a height of its own: the
//!   ground under it, and over a road, a river or a lower deck the clearance
//!   that asks (clearance.rs). A clearance raises the deck about it, falling
//!   off at the steepest a deck climbs.
//! - **Coming down again.** A clearance is held to what a deck could climb to
//!   from every end it has, so it never leaves a step at an end.
//!
//! The top is the higher of the line and the raised clearances: on an
//! approach, where there is no line, its ground or the deck coming down to
//! it, whichever is higher. A structure that reaches the ground nowhere — a
//! loop of joints — has no line, and is its clearances.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::deck::clearance::MAX_SLOPE;

/// What one structure asks, by point.
pub struct Problem<'a> {
    pub adj: &'a [Vec<(u32, f64)>],
    /// The top each point needs at least, in metres: its ground, or what
    /// passes under it with its clearance.
    pub need: &'a [f64],
    /// Points where the deck meets the ground, and the ground there.
    pub ends: &'a [(u32, f64)],
    /// The ground at each point, in metres.
    pub ground: &'a [f64],
    /// Whether each point is carried — a bridge's or a tunnel's own — rather
    /// than on an approach.
    pub carried: &'a [bool],
}

/// Each point's top, in metres.
pub fn solve(p: &Problem) -> Vec<f64> {
    let n = p.need.len();
    // The highest each point can be and still come down to every end.
    let cap = spread(p.adj, p.ends.iter().copied(), |v, d| v + MAX_SLOPE * d, Ordering::Less);
    // Each clearance the carried part asks, held to the cap, and the deck
    // raised about it. An approach asks only its own ground, which raises
    // nothing past it.
    let held = (0..n).filter(|&v| p.carried[v]).map(|v| (v as u32, p.need[v].min(cap[v])));
    let raised = spread(p.adj, held, |v, d| v - MAX_SLOPE * d, Ordering::Greater);
    let line = between_ends(p);
    (0..n).map(|v| if p.carried[v] { line[v].max(raised[v]) } else { p.need[v].max(raised[v]) }).collect()
}

/// The straight line between the carried part's ends, at each carried
/// point: the ends' grounds weighted by the inverse of each one's distance
/// along it. Its ends are the structure's own ends and the points where it
/// joins an approach.
fn between_ends(p: &Problem) -> Vec<f64> {
    let n = p.need.len();
    let carried: Vec<Vec<(u32, f64)>> = (0..n)
        .map(|v| {
            if p.carried[v] {
                p.adj[v].iter().copied().filter(|&(w, _)| p.carried[w as usize]).collect()
            } else {
                Vec::new()
            }
        })
        .collect();
    let joints = (0..n)
        .filter(|&v| p.carried[v] && p.adj[v].iter().any(|&(w, _)| !p.carried[w as usize]))
        .map(|v| (v as u32, p.ground[v]));
    let ends: Vec<(u32, f64)> = p.ends.iter().copied().filter(|&(e, _)| p.carried[e as usize]).chain(joints).collect();
    straight(&carried, &ends)
}

/// The straight line between `ends` over a graph, at each point: the ends'
/// heights weighted by the inverse of each one's distance along it, which on
/// a single span is exactly the straight line. A point no end reaches has
/// none.
pub fn straight(adj: &[Vec<(u32, f64)>], ends: &[(u32, f64)]) -> Vec<f64> {
    let n = adj.len();
    let (mut sum, mut weight) = (vec![0.0; n], vec![0.0; n]);
    let mut at_end: Vec<Option<f64>> = vec![None; n];
    for &(end, ground) in ends {
        at_end[end as usize] = Some(ground);
        let d = spread(adj, [(end, 0.0)], |v, d| v + d, Ordering::Less);
        for v in 0..n {
            if d[v].is_finite() && d[v] > 0.0 {
                sum[v] += ground / d[v];
                weight[v] += 1.0 / d[v];
            }
        }
    }
    (0..n).map(|v| at_end[v].unwrap_or(if weight[v] > 0.0 { sum[v] / weight[v] } else { f64::NEG_INFINITY })).collect()
}

/// A value spread from `sources` over the graph by `along` per metre: each
/// point the best — least for `Less`, greatest for `Greater` — of what any
/// source reaches it with. Dijkstra, best first.
fn spread(adj: &[Vec<(u32, f64)>], sources: impl IntoIterator<Item = (u32, f64)>, along: impl Fn(f64, f64) -> f64, best: Ordering) -> Vec<f64> {
    let worst = if best == Ordering::Less { f64::INFINITY } else { f64::NEG_INFINITY };
    let mut value = vec![worst; adj.len()];
    let mut heap = BinaryHeap::new();
    for (v, x) in sources {
        if x.partial_cmp(&value[v as usize]) == Some(best) {
            value[v as usize] = x;
            heap.push(Entry { value: x, node: v, best });
        }
    }
    while let Some(Entry { value: x, node, .. }) = heap.pop() {
        if x != value[node as usize] {
            continue;
        }
        for &(next, d) in &adj[node as usize] {
            let y = along(x, d);
            if y.partial_cmp(&value[next as usize]) == Some(best) {
                value[next as usize] = y;
                heap.push(Entry { value: y, node: next, best });
            }
        }
    }
    value
}

/// A heap entry whose best value pops first.
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
