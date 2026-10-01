// pxl-geo-bake/deck/profile.rs

//! A structure's deck profile, solved on the graph of its points.
//!
//! Three things decide a deck's top at a point, all along the structure and
//! never across it:
//!
//! - **The line between where it meets the ground.** A bridge is carried
//!   from end to end, so where nothing is under it, it runs straight between
//!   its ends' ground — over a valley as much as over a flat. On a single
//!   span that is the straight line; where spans branch, each stretch is as
//!   gentle as the ends around it allow (`line`). Only the
//!   carried part has a line: an approach is a way on the ground, and its
//!   ends are where it joins the bridge, so the line never runs from one
//!   approach's far end to the other's across whatever lies between.
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
//!
//! So the top is never steeper than `MAX_SLOPE` along the carried part: the
//! raised clearances are no steeper by how they spread, the line no steeper
//! than its ends make it, and the higher of two such is no steeper either —
//! except on a stretch between two ends whose own ground is steeper than
//! that apart, which no deck could meet, and which the profile marks as
//! forced (`Profile::forced`) rather than leaving it to look like the
//! solver's.

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

/// A structure's tops.
pub struct Profile {
    /// Each point's top, in metres.
    pub top: Vec<f64>,
    /// Each point the line had to make steeper than `MAX_SLOPE`, because
    /// the ends it runs between are steeper than that apart.
    pub forced: Vec<bool>,
}

/// Each point's top.
pub fn solve(p: &Problem) -> Profile {
    let n = p.need.len();
    // The highest each point can be and still come down to every end.
    let cap = spread(p.adj, p.ends.iter().copied(), |v, d| v + MAX_SLOPE * d, Ordering::Less);
    // Each clearance the carried part asks, held to the cap, and the deck
    // raised about it. An approach asks only its own ground, which raises
    // nothing past it.
    let held = (0..n).filter(|&v| p.carried[v]).map(|v| (v as u32, p.need[v].min(cap[v])));
    let raised = spread(p.adj, held, |v, d| v - MAX_SLOPE * d, Ordering::Greater);
    let (line, slope) = between_ends(p);
    let top = (0..n).map(|v| if p.carried[v] { line[v].max(raised[v]) } else { p.need[v].max(raised[v]) }).collect();
    let forced = slope.iter().map(|&s| s > MAX_SLOPE + 1e-9).collect();
    Profile { top, forced }
}

/// The line between the carried part's ends, at each carried point, and how
/// steep the stretch is that set it. Its ends are the structure's own ends
/// and the points where it joins an approach.
fn between_ends(p: &Problem) -> (Vec<f64>, Vec<f64>) {
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
    line(&carried, &ends)
}

/// The line through `ends` over a graph, and at each point how steep the
/// stretch is that set it: the steepest stretch first, then the steepest of
/// what is left. Of every path between two points already set that passes
/// only points not yet set, the steepest is laid straight and set, and each
/// part of the graph it leaves is done the same way on its own, until every
/// point an end reaches is set; a part that touches one set point only is
/// that point's height. This is the line that is as gentle as its ends allow
/// on every stretch, not only on the steepest — the absolutely minimizing
/// Lipschitz extension, by the steepest-path algorithm of Lazarus, Loeb,
/// Propp and Ullman as Kyng, Rao, Sachdeva and Spielman put it to graphs —
/// so on a single span it is the straight line, where spans meet the
/// junction stands where the steepest pair of ends puts it, and a pair of
/// ends steeper than a deck climbs makes only the stretch between them
/// steep. A point no end reaches has none.
fn line(adj: &[Vec<(u32, f64)>], ends: &[(u32, f64)]) -> (Vec<f64>, Vec<f64>) {
    let n = adj.len();
    let mut height: Vec<Option<f64>> = vec![None; n];
    let mut slope = vec![0.0; n];
    for &(e, h) in ends {
        height[e as usize] = Some(h);
    }
    loop {
        // The steepest path between two set points through unset ones, from
        // each set point that has an unset neighbour.
        let mut best: Option<(f64, Vec<u32>)> = None;
        for a in 0..n {
            let Some(ha) = height[a] else { continue };
            if !adj[a].iter().any(|&(w, _)| height[w as usize].is_none()) {
                continue;
            }
            let (dist, from) = through_unset(adj, &height, a as u32);
            for b in 0..n {
                let (Some(hb), Some(d)) = (height[b], dist[b]) else { continue };
                if b == a || from[b] == a as u32 || d <= 0.0 {
                    continue;
                }
                let s = (ha - hb).abs() / d;
                if best.as_ref().is_none_or(|(bs, _)| s > *bs) {
                    let mut path = vec![b as u32];
                    while *path.last().unwrap() != a as u32 {
                        path.push(from[*path.last().unwrap() as usize]);
                    }
                    best = Some((s, path));
                }
            }
        }
        let Some((s, path)) = best else { break };
        // Laid straight between its two set ends, by distance along it.
        let (hb, ha) = (height[path[0] as usize].unwrap(), height[*path.last().unwrap() as usize].unwrap());
        let along: Vec<f64> = path
            .windows(2)
            .scan(0.0, |d, w| {
                *d += adj[w[0] as usize].iter().filter(|&&(x, _)| x == w[1]).map(|&(_, l)| l).fold(f64::INFINITY, f64::min);
                Some(*d)
            })
            .collect();
        let total = *along.last().unwrap();
        for (i, &v) in path.iter().enumerate().skip(1).take(path.len() - 2) {
            height[v as usize] = Some(hb + (ha - hb) * along[i - 1] / total);
            slope[v as usize] = s;
        }
    }
    // What is left touches one set point at most: it is that point's height.
    let mut queue: Vec<u32> = (0..n as u32).filter(|&v| height[v as usize].is_some()).collect();
    while let Some(v) = queue.pop() {
        for &(w, _) in &adj[v as usize] {
            if height[w as usize].is_none() {
                height[w as usize] = height[v as usize];
                queue.push(w);
            }
        }
    }
    (height.into_iter().map(|h| h.unwrap_or(f64::NEG_INFINITY)).collect(), slope)
}

/// Distances from set point `a` to every point, over unset points only — a
/// set point is reached but not passed through — and each point's
/// predecessor on the way.
fn through_unset(adj: &[Vec<(u32, f64)>], height: &[Option<f64>], a: u32) -> (Vec<Option<f64>>, Vec<u32>) {
    let n = adj.len();
    let mut dist = vec![None; n];
    let mut from = vec![u32::MAX; n];
    let mut heap = BinaryHeap::new();
    dist[a as usize] = Some(0.0);
    heap.push(Entry {
        value: 0.0,
        node: a,
        best: Ordering::Less,
    });
    while let Some(Entry { value, node, .. }) = heap.pop() {
        if dist[node as usize] != Some(value) || (node != a && height[node as usize].is_some()) {
            continue;
        }
        for &(next, d) in &adj[node as usize] {
            let y = value + d;
            if dist[next as usize].is_none_or(|x| y < x) {
                dist[next as usize] = Some(y);
                from[next as usize] = node;
                heap.push(Entry {
                    value: y,
                    node: next,
                    best: Ordering::Less,
                });
            }
        }
    }
    (dist, from)
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
