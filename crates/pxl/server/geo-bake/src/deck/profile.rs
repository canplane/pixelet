// pxl-geo-bake/deck/profile.rs

//! A structure's road surface on the graph of its points, in `q`: a deck's
//! height, a bore's depth. Each point's need is hard: where `PREFERRED_SLOPE`
//! cannot reach it from the ends, the line between ends and held points is
//! laid as gently as the needs allow; elsewhere a need is raised at
//! `PREFERRED_SLOPE`. A step steeper than `PREFERRED_SLOPE` is marked forced.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::deck::param::PREFERRED_SLOPE;

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
    let cap = spread(p.adj, p.ends.iter().copied(), |v, d| v + PREFERRED_SLOPE * d, Ordering::Less);
    let held = (0..n).filter(|&v| p.carried[v]).map(|v| (v as u32, p.need[v].min(cap[v])));
    let raised = spread(p.adj, held, |v, d| v - PREFERRED_SLOPE * d, Ordering::Greater);
    // Points the grade cannot reach are held, in each stretch between set
    // points the one furthest under its line: over a chord that is a vertex of
    // the upper hull, so the line is a taut string over the needs, not a
    // staircase along them. Stretches are apart, so each round holds one in each.
    let mut unreached: Vec<(u32, f64)> = Vec::new();
    loop {
        let (line, slope, set) = between_ends(p, unreached.iter().copied());
        let top: Vec<f64> = (0..n).map(|v| if p.carried[v] { line[v].max(raised[v]) } else { p.need[v].max(raised[v]) }).collect();
        let under = |v: usize| p.need[v] - top[v];
        let stretch = stretches(p, &set);
        let mut worst: HashMap<usize, usize> = HashMap::new();
        for v in (0..n).filter(|&v| p.carried[v] && under(v) > 1e-9) {
            let w = worst.entry(stretch[v]).or_insert(v);
            if under(v) > under(*w) {
                *w = v;
            }
        }
        if worst.is_empty() {
            let forced = slope.iter().map(|&s| s > PREFERRED_SLOPE + 1e-9).collect();
            return Profile { top, forced };
        }
        for v in worst.into_values() {
            assert!(!unreached.iter().any(|&(u, _)| u as usize == v), "a held point under its need: a structure's form is broken");
            unreached.push((v as u32, p.need[v]));
        }
    }
}

/// Each point's stretch: carried points joined without passing a set point;
/// a set point is a stretch of its own.
fn stretches(p: &Problem, set: &[bool]) -> Vec<usize> {
    let n = p.need.len();
    let mut id = vec![usize::MAX; n];
    for s in 0..n {
        if id[s] != usize::MAX {
            continue;
        }
        id[s] = s;
        if set[s] || !p.carried[s] {
            continue;
        }
        let mut stack = vec![s];
        while let Some(v) = stack.pop() {
            for &(w, _) in &p.adj[v] {
                let w = w as usize;
                if id[w] == usize::MAX && p.carried[w] && !set[w] {
                    id[w] = s;
                    stack.push(w);
                }
            }
        }
    }
    id
}

fn between_ends(p: &Problem, held: impl Iterator<Item = (u32, f64)>) -> (Vec<f64>, Vec<f64>, Vec<bool>) {
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
    let mut ends: Vec<(u32, f64)> = p.ends.iter().copied().filter(|&(e, _)| p.carried[e as usize]).chain(joints).collect();
    // A held point wins over an end or a joint on the same point.
    for (v, h) in held {
        ends.retain(|&(e, _)| e != v);
        ends.push((v, h));
    }
    let mut set = vec![false; n];
    for &(e, _) in &ends {
        set[e as usize] = true;
    }
    let (line, slope) = line(&carried, &ends);
    (line, slope, set)
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
