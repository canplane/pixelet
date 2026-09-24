// pxl-kernel/layer/poi/store.rs

use crate::global::Global;
use crate::io::{demand::Category, policy, sched::declare};
use crate::shell::config::env::APP_ENV;
use crate::shell::setting::store::settings_store;
use crate::world::change::request_world_tick;
use pxl_spec::poi::{wire, xy, Node, Summary};

#[derive(Clone, Copy)]
pub struct Limits {
    pub candidates: usize,
    pub resident_bytes: usize,
}

pub const LIMITS: Limits = Limits { candidates: 8, resident_bytes: 4096 };
// The decoder bounds the allocation even before a requested packet arrives.
const NODE_RESERVATION: usize = std::mem::size_of::<Node>() + xy::CHILDREN * std::mem::size_of::<Summary>();

pub struct Relevance {
    pub distance: f64,
    pub refine: bool,
}

struct Request {
    subject: u64,
    expected: Option<Summary>,
    distance: f64,
    revision: Option<u64>,
}

#[derive(Default)]
pub struct Store {
    // Oldest to most recently used; only this view's traversal is pinned.
    nodes: Vec<Node>,
    pub cut: Vec<Summary>,
    wanted: Vec<Request>,
}

impl Store {
    pub fn bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<Node>() + self.nodes.iter().map(|n| n.children.capacity() * std::mem::size_of::<Summary>()).sum::<usize>()
    }

    pub fn pending_count(&self) -> usize {
        self.wanted.len()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn adopt(&mut self, subject: u64, node: Node, budget: usize) -> Result<(), &'static str> {
        let Some(request) = self.wanted.iter().find(|r| r.subject == subject) else {
            return Ok(());
        };
        wire::validate(&node)?;
        if node.summary.key != subject {
            return Err("summary address mismatch");
        }
        if let Some(revision) = request.revision {
            if node.revision != revision {
                let root = subject >> (2 * (xy::depth(subject).unwrap() - xy::ROOT_DEPTH));
                self.nodes.retain(|n| !xy::contains(root, n.summary.key));
                self.nodes.shrink_to_fit();
                self.wanted.retain(|r| !xy::contains(root, r.subject));
                self.cut.retain(|s| !xy::contains(root, s.key));
                return Err("refinement snapshot changed; resolve region again");
            }
        }
        if request.expected.is_some_and(|expected| node.summary != expected) {
            return Err("refinement coverage mismatch");
        }
        let slot = usize::from(self.nodes.len() == self.nodes.capacity()) * std::mem::size_of::<Node>();
        let bytes = slot + node.children.capacity() * std::mem::size_of::<Summary>();
        if self.bytes() + bytes > budget {
            return Err("summary residency budget");
        }
        self.nodes.reserve_exact(1);
        self.nodes.push(node);
        self.wanted.retain(|r| r.subject != subject);
        Ok(())
    }

    fn make_room(&mut self, extra: usize, budget: usize, pinned: &[u64]) -> bool {
        while self.bytes() + extra > budget {
            let Some(i) = self.nodes.iter().position(|n| !pinned.contains(&n.summary.key)) else {
                return false;
            };
            self.nodes.remove(i);
            self.nodes.shrink_to_fit();
        }
        true
    }

    /// The cut covers only the current view. Each replacement uses every
    /// relevant child of a validated node, or leaves the parent intact. Screen
    /// collisions never participate in this choice or change a summary's count.
    pub fn select(&mut self, roots: &[(u64, f64)], assess: impl Fn(&Summary) -> Option<Relevance>, limits: Limits) {
        self.cut.clear();
        self.wanted.clear();
        let roots = &roots[..roots.len().min(limits.candidates)];
        let mut pinned: Vec<_> = roots.iter().map(|r| r.0).collect();
        if !self.make_room(0, limits.resident_bytes, &pinned) {
            self.nodes.clear();
            self.nodes.shrink_to_fit();
        }
        for &(key, distance) in roots {
            if let Some(root) = self.nodes.iter().find(|n| n.summary.key == key) {
                if root.summary.count != 0 && assess(&root.summary).is_some() {
                    self.cut.push(root.summary);
                }
            } else if self.make_room((self.wanted.len() + 1) * NODE_RESERVATION, limits.resident_bytes, &pinned) {
                self.wanted.push(Request {
                    subject: key,
                    expected: None,
                    distance,
                    revision: None,
                });
            }
        }
        let mut requested = Vec::new();
        loop {
            let next = self
                .cut
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    if !s.refinable || requested.contains(&s.key) {
                        return None;
                    }
                    if let Some(node) = self.nodes.iter().find(|n| n.summary.key == s.key) {
                        let count = node.children.iter().filter(|c| assess(c).is_some()).count();
                        if self.cut.len() - 1 + count > limits.candidates {
                            return None;
                        }
                    }
                    let r = assess(s)?;
                    r.refine.then_some((i, r.distance, s.key))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)));
            let Some((i, distance, key)) = next else { break };
            if let Some(at) = self.nodes.iter().position(|n| n.summary.key == key) {
                let children: Vec<_> = self.nodes[at].children.iter().filter(|s| assess(s).is_some()).copied().collect();
                pinned.push(key);
                self.cut.splice(i..=i, children);
                if at != 0 {
                    let used = self.nodes.remove(at);
                    self.nodes.push(used);
                }
            } else {
                requested.push(key);
                // Current traversal ancestors stay usable. Cached branches
                // outside that path yield their bytes to today's demand.
                let reservation = (self.wanted.len() + 1) * NODE_RESERVATION;
                if self.make_room(reservation, limits.resident_bytes, &pinned) {
                    self.wanted.push(Request {
                        subject: key,
                        expected: Some(self.cut[i]),
                        distance,
                        revision: self.nodes.iter().find(|n| xy::contains(n.summary.key, key)).map(|n| n.revision),
                    });
                }
            }
        }
    }
}

static STORE: Global<Store> = Global::new();
pub fn store() -> &'static mut Store {
    STORE.get(Store::default)
}

pub fn enabled() -> bool {
    APP_ENV() != "production" && settings_store().show_poi
}

pub fn demand() {
    let wants: Vec<_> = store().wanted.iter().map(|r| (r.subject, policy::poi(r.distance))).collect();
    declare(Category::Poi, &wants);
}

pub fn adopt(subject: u64, bytes: &[u8]) -> Result<(), &'static str> {
    let node = wire::decode(bytes)?;
    let result = store().adopt(subject, node, LIMITS.resident_bytes);
    demand();
    request_world_tick();
    result
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
