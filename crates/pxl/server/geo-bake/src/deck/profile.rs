// pxl-geo-bake/deck/profile.rs

//! A structure's road surface in `q`: a deck's height, a bore's depth.
//! Needs are hard, `PREFERRED_SLOPE` is a preference.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use crate::deck::param::PREFERRED_SLOPE;

pub struct Problem<'a> {
    pub adj: &'a [Vec<(u32, f64)>],
    pub need: &'a [f64],
    pub ends: &'a [(u32, f64)],
    pub ground: &'a [f64],
    pub carried: &'a [bool],
    /// Joint first, distance along.
    pub approaches: &'a [&'a [(u32, f64)]],
}

pub struct Profile {
    pub top: Vec<f64>,
    pub forced: Vec<bool>,
}

pub fn solve(p: &Problem) -> Profile {
    let n = p.need.len();
    let adj = carried_adj(p);
    // highest that still comes down to every end
    let cap = spread(&adj, ends_of(p), |v, d| v + PREFERRED_SLOPE * d, Ordering::Less);
    let held = (0..n).filter(|&v| p.carried[v]).map(|v| (v as u32, p.need[v].min(cap[v])));
    let raised = spread(&adj, held, |v, d| v - PREFERRED_SLOPE * d, Ordering::Greater);
    // hold the worst short point per stretch: a hull vertex, so no staircase
    let mut unreached: Vec<(u32, f64)> = Vec::new();
    loop {
        let (line, slope, set) = between_ends(p, &adj, unreached.iter().copied());
        let mut top: Vec<f64> = (0..n).map(|v| if p.carried[v] { line[v].max(raised[v]) } else { p.ground[v] }).collect();
        let under = |v: usize| p.need[v] - top[v];
        let stretch = stretches(p, &adj, &set);
        let mut worst: BTreeMap<usize, usize> = BTreeMap::new();
        for v in (0..n).filter(|&v| p.carried[v] && under(v) > 1e-9) {
            let w = worst.entry(stretch[v]).or_insert(v);
            if under(v) > under(*w) {
                *w = v;
            }
        }
        if worst.is_empty() {
            let mut forced: Vec<bool> = slope.iter().map(|&s| s > PREFERRED_SLOPE + 1e-9).collect();
            for chain in p.approaches.iter().filter(|c| c.len() >= 2 && p.carried[c[0].0 as usize]) {
                approach(&mut top, &mut forced, p.ground, chain);
            }
            return Profile { top, forced };
        }
        for v in worst.into_values() {
            assert!(!unreached.iter().any(|&(u, _)| u as usize == v), "a held point under its need: a structure's form is broken");
            unreached.push((v as u32, p.need[v]));
        }
    }
}

/// Carried points joined without a set point between; a set point alone.
fn stretches(p: &Problem, adj: &[Vec<(u32, f64)>], set: &[bool]) -> Vec<usize> {
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
            for &(w, _) in &adj[v] {
                let w = w as usize;
                if id[w] == usize::MAX && !set[w] {
                    id[w] = s;
                    stack.push(w);
                }
            }
        }
    }
    id
}

/// Straight from the joint to the first ground in grade, else the gentlest:
/// nothing before it is under the line.
fn approach(top: &mut [f64], forced: &mut [bool], ground: &[f64], chain: &[(u32, f64)]) {
    let (joint, d0) = chain[0];
    let from = top[joint as usize];
    let rest = &chain[1..];
    let grade = |i: usize| (from - ground[rest[i].0 as usize]) / (rest[i].1 - d0);
    let gentlest = || (0..rest.len()).min_by(|&a, &b| grade(a).total_cmp(&grade(b)));
    let Some(contact) = (0..rest.len()).find(|&i| grade(i) <= PREFERRED_SLOPE).or_else(gentlest) else { return };
    let m = grade(contact);
    for &(v, d) in &rest[..contact] {
        top[v as usize] = top[v as usize].max(from - m * (d - d0));
        forced[v as usize] |= m > PREFERRED_SLOPE + 1e-9;
    }
}

fn carried_adj(p: &Problem) -> Vec<Vec<(u32, f64)>> {
    (0..p.need.len())
        .map(|v| if p.carried[v] { p.adj[v].iter().copied().filter(|&(w, _)| p.carried[w as usize]).collect() } else { Vec::new() })
        .collect()
}

/// Free ends, and joints at their need.
fn ends_of<'a>(p: &'a Problem) -> impl Iterator<Item = (u32, f64)> + 'a {
    let joints = (0..p.need.len()).filter(|&v| p.carried[v] && p.adj[v].iter().any(|&(w, _)| !p.carried[w as usize])).map(|v| (v as u32, p.need[v]));
    p.ends.iter().copied().filter(|&(e, _)| p.carried[e as usize]).chain(joints)
}

fn between_ends(p: &Problem, adj: &[Vec<(u32, f64)>], held: impl Iterator<Item = (u32, f64)>) -> (Vec<f64>, Vec<f64>, Vec<bool>) {
    let n = p.need.len();
    let mut ends: Vec<(u32, f64)> = ends_of(p).collect();
    // held wins over an end on its point
    for (v, h) in held {
        ends.retain(|&(e, _)| e != v);
        ends.push((v, h));
    }
    let mut set = vec![false; n];
    for &(e, _) in &ends {
        set[e as usize] = true;
    }
    let (line, slope) = line(adj, &ends);
    (line, slope, set)
}

/// Absolutely minimizing Lipschitz extension through `ends`: steepest path
/// first (Kyng, Rao, Sachdeva, Spielman). Also each point's slope.
fn line(adj: &[Vec<(u32, f64)>], ends: &[(u32, f64)]) -> (Vec<f64>, Vec<f64>) {
    let n = adj.len();
    let mut height: Vec<Option<f64>> = vec![None; n];
    let mut slope = vec![0.0; n];
    for &(e, h) in ends {
        height[e as usize] = Some(h);
    }
    // runs between set points are independent
    let mut search = Search::new(n);
    for run in runs(adj, &height) {
        loop {
            let mut from_set: Vec<u32> = run.iter().filter(|&&v| height[v as usize].is_none()).flat_map(|&v| adj[v as usize].iter().map(|&(w, _)| w)).filter(|&w| height[w as usize].is_some()).collect();
            from_set.sort_unstable();
            from_set.dedup();
            let mut best: Option<(f64, Vec<u32>)> = None;
            for &a in &from_set {
                let ha = height[a as usize].unwrap();
                search.run(adj, &height, a);
                let mut reached = search.reached.clone();
                reached.sort_unstable();
                for b in reached {
                    let (Some(hb), d) = (height[b as usize], search.dist[b as usize]) else { continue };
                    if b == a || search.from[b as usize] == a || d <= 0.0 {
                        continue;
                    }
                    let s = (ha - hb).abs() / d;
                    if best.as_ref().is_none_or(|(bs, _)| s > *bs) {
                        let mut path = vec![b];
                        while *path.last().unwrap() != a {
                            path.push(search.from[*path.last().unwrap() as usize]);
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
    }
    // the rest touches one set point: its height
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

fn runs(adj: &[Vec<(u32, f64)>], height: &[Option<f64>]) -> Vec<Vec<u32>> {
    let mut seen = vec![false; adj.len()];
    let mut out = Vec::new();
    for s in 0..adj.len() {
        if seen[s] || height[s].is_some() {
            continue;
        }
        seen[s] = true;
        let (mut run, mut stack) = (vec![s as u32], vec![s]);
        while let Some(v) = stack.pop() {
            for &(w, _) in &adj[v] {
                let w = w as usize;
                if !seen[w] && height[w].is_none() {
                    seen[w] = true;
                    run.push(w as u32);
                    stack.push(w);
                }
            }
        }
        out.push(run);
    }
    out
}

/// Distances through unset points; resets only what it reached.
struct Search {
    dist: Vec<f64>,
    from: Vec<u32>,
    reached: Vec<u32>,
}

impl Search {
    fn new(n: usize) -> Search {
        Search {
            dist: vec![f64::INFINITY; n],
            from: vec![u32::MAX; n],
            reached: Vec::new(),
        }
    }

    fn run(&mut self, adj: &[Vec<(u32, f64)>], height: &[Option<f64>], a: u32) {
        for &v in &self.reached {
            self.dist[v as usize] = f64::INFINITY;
            self.from[v as usize] = u32::MAX;
        }
        self.reached.clear();
        let mut heap = BinaryHeap::new();
        self.dist[a as usize] = 0.0;
        self.reached.push(a);
        heap.push(Entry { value: 0.0, node: a, best: Ordering::Less });
        while let Some(Entry { value, node, .. }) = heap.pop() {
            if self.dist[node as usize] != value || (node != a && height[node as usize].is_some()) {
                continue;
            }
            for &(next, d) in &adj[node as usize] {
                let y = value + d;
                if y < self.dist[next as usize] {
                    if self.dist[next as usize] == f64::INFINITY {
                        self.reached.push(next);
                    }
                    self.dist[next as usize] = y;
                    self.from[next as usize] = node;
                    heap.push(Entry { value: y, node: next, best: Ordering::Less });
                }
            }
        }
    }
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
