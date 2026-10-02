// pxl-geo-bake/deck/profile.rs

//! A structure's deck profile on the graph of its points: the higher of the
//! line between its ground ends and each clearance raised at `MAX_SLOPE`, the
//! clearances capped so the deck can come down to every end. Only where two
//! ends' own ground is steeper than `MAX_SLOPE` is a step left, marked forced.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::deck::clearance::MAX_SLOPE;

pub struct Problem<'a> {
    pub adj: &'a [Vec<(u32, f64)>],
    pub need: &'a [f64],
    pub ends: &'a [(u32, f64)],
    pub ground: &'a [f64],
    pub carried: &'a [bool],
}

pub struct Profile {
    pub top: Vec<f64>,
    pub forced: Vec<bool>,
}

pub fn solve(p: &Problem) -> Profile {
    let n = p.need.len();
    // The highest each point can be and still come down to every end.
    let cap = spread(p.adj, p.ends.iter().copied(), |v, d| v + MAX_SLOPE * d, Ordering::Less);
    let held = (0..n).filter(|&v| p.carried[v]).map(|v| (v as u32, p.need[v].min(cap[v])));
    let raised = spread(p.adj, held, |v, d| v - MAX_SLOPE * d, Ordering::Greater);
    let (line, slope) = between_ends(p);
    let top = (0..n).map(|v| if p.carried[v] { line[v].max(raised[v]) } else { p.need[v].max(raised[v]) }).collect();
    let forced = slope.iter().map(|&s| s > MAX_SLOPE + 1e-9).collect();
    Profile { top, forced }
}

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

/// The absolutely minimizing Lipschitz extension through `ends`, by the
/// steepest-path algorithm (Lazarus et al., as Kyng, Rao, Sachdeva and
/// Spielman put it to graphs): the steepest path between set points is laid
/// straight, repeatedly. Also returns each point's slope.
fn line(adj: &[Vec<(u32, f64)>], ends: &[(u32, f64)]) -> (Vec<f64>, Vec<f64>) {
    let n = adj.len();
    let mut height: Vec<Option<f64>> = vec![None; n];
    let mut slope = vec![0.0; n];
    for &(e, h) in ends {
        height[e as usize] = Some(h);
    }
    loop {
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
    // What is left touches one set point at most: it takes that point's height.
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
