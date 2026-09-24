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
    // Oldest to most recently used. Region roots, not traversal paths, are pinned.
    nodes: Vec<Node>,
    pub cut: Vec<Summary>,
    wanted: Vec<Request>,
    // Relevant child counts for the current view, independent of packet residency.
    refinement_sizes: Vec<(u64, usize)>,
    resolved_roots: Vec<u64>,
}

impl Store {
    pub fn revision(&self, key: u64) -> Option<u64> {
        self.nodes.iter().find(|n| xy::contains(n.summary.key, key)).map(|n| n.revision)
    }

    pub fn invalidate(&mut self, key: u64) {
        let root = key >> (2 * (xy::depth(key).unwrap() - xy::ROOT_DEPTH));
        self.nodes.retain(|n| !xy::contains(root, n.summary.key));
        self.wanted.retain(|r| !xy::contains(root, r.subject));
        self.cut.retain(|s| !xy::contains(root, s.key));
        self.resolved_roots.retain(|key| *key != root);
    }

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
                self.invalidate(subject);
                return Err("refinement snapshot changed; resolve region again");
            }
        }
        if request.expected.is_some_and(|expected| node.summary != expected) {
            return Err("refinement coverage mismatch");
        }
        // A returning region may still have descendants from an evicted root.
        // A freshly resolved root owns the revision of all such residency.
        if request.expected.is_none() && self.nodes.iter().any(|n| xy::contains(subject, n.summary.key) && n.revision != node.revision) {
            self.nodes.retain(|n| !xy::contains(subject, n.summary.key));
            self.nodes.shrink_to_fit();
            self.cut.retain(|s| !xy::contains(subject, s.key));
            self.resolved_roots.retain(|key| *key != subject);
            self.wanted.retain(|r| r.subject == subject || !xy::contains(subject, r.subject));
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
    pub fn select(&mut self, view_changed: bool, roots: &[(u64, f64)], assess: impl Fn(u64, [f64; 6]) -> Option<Relevance>, limits: Limits) {
        let previous = std::mem::take(&mut self.cut);
        self.wanted.clear();
        if view_changed {
            self.refinement_sizes.clear();
            self.resolved_roots.clear();
        }
        let roots = &roots[..roots.len().min(limits.candidates)];
        let pinned: Vec<_> = roots.iter().map(|r| r.0).collect();
        self.resolved_roots.retain(|key| pinned.contains(key));
        if !self.make_room(0, limits.resident_bytes, &pinned) {
            self.nodes.clear();
            self.nodes.shrink_to_fit();
        }
        // This cut was already proven complete for this exact view. Rebuilding
        // from roots would require evicted ancestry to prove the same fact.
        if !view_changed && previous.len() <= limits.candidates {
            self.cut.extend(previous.iter().filter(|s| roots.iter().any(|r| xy::contains(r.0, s.key))).copied());
        }
        for &(key, distance) in roots {
            if self.resolved_roots.contains(&key) {
                continue;
            }
            if let Some(root) = self.nodes.iter().find(|n| n.summary.key == key) {
                self.resolved_roots.push(key);
                if root.summary.count != 0 && assess(root.summary.key, root.summary.bounds).is_some() {
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
        // Unknown roots may become visible when their packets arrive. Their
        // slots belong to this view already, not to response arrival order.
        let unresolved = roots.iter().filter(|r| !self.nodes.iter().any(|n| n.summary.key == r.0)).count();
        let available = limits.candidates.saturating_sub(unresolved);
        let mut requested = Vec::new();
        loop {
            self.refinement_sizes.retain(|(key, _)| self.cut.iter().any(|s| s.key == *key));
            for summary in &self.cut {
                if let Some(node) = self.nodes.iter().find(|n| n.summary.key == summary.key) {
                    let count = node.children.iter().filter(|c| assess(c.key, c.bounds).is_some()).count();
                    if let Some(size) = self.refinement_sizes.iter_mut().find(|(key, _)| *key == summary.key) {
                        size.1 = count;
                    } else {
                        self.refinement_sizes.push((summary.key, count));
                    }
                }
            }
            let next = self
                .cut
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    if !s.refinable || requested.contains(&s.key) {
                        return None;
                    }
                    if self.refinement_sizes.iter().any(|&(key, size)| key == s.key && self.cut.len() - 1 + size > available) {
                        return None;
                    }
                    let r = assess(s.key, s.bounds)?;
                    r.refine.then_some((i, r.distance, s.key))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)));
            let Some((i, distance, key)) = next else { break };
            if let Some(at) = self.nodes.iter().position(|n| n.summary.key == key) {
                let children: Vec<_> = self.nodes[at].children.iter().filter(|s| assess(s.key, s.bounds).is_some()).copied().collect();
                self.cut.splice(i..=i, children);
                if at != 0 {
                    let used = self.nodes.remove(at);
                    self.nodes.push(used);
                }
            } else {
                let parent = self.cut[i];
                let revision = self.nodes.iter().find(|n| xy::contains(n.summary.key, key)).map(|n| n.revision);
                // A missing ancestor is not required when its entire visible
                // population lies in a complete known descendant cut. The last
                // cut also keeps an in-flight frontier usable after eviction.
                let frontier: Vec<_> = previous
                    .iter()
                    .filter(|s| s.key != parent.key && xy::contains(parent.key, s.key) && assess(s.key, s.bounds).is_some())
                    .copied()
                    .collect();
                if !frontier.is_empty() && self.cut.len() - 1 + frontier.len() <= available && visible_within(parent, &frontier, &assess) {
                    self.cut.splice(i..=i, frontier);
                    continue;
                }
                let resident: Vec<_> = self
                    .nodes
                    .iter()
                    .filter(|n| Some(n.revision) == revision && n.summary.key != parent.key && xy::contains(parent.key, n.summary.key))
                    .map(|n| n.summary)
                    .collect();
                let frontier: Vec<_> = resident
                    .iter()
                    .filter(|s| assess(s.key, s.bounds).is_some() && !resident.iter().any(|other| other.key != s.key && xy::contains(other.key, s.key)))
                    .copied()
                    .collect();
                if !frontier.is_empty() && self.cut.len() - 1 + frontier.len() <= available && visible_within(parent, &frontier, &assess) {
                    self.cut.splice(i..=i, frontier);
                    continue;
                }
                requested.push(key);
                let reservation = (self.wanted.len() + 1) * NODE_RESERVATION;
                if self.make_room(reservation, limits.resident_bytes, &pinned) {
                    self.wanted.push(Request {
                        subject: key,
                        expected: Some(self.cut[i]),
                        distance,
                        revision,
                    });
                    // Response timing must not let a lower-priority branch
                    // spend the slots this nearer refinement will need.
                    break;
                }
            }
        }
    }
}

// Each excluded sibling is an address envelope clipped to the ancestor's
// canonical bounds, not an invented population or aggregate. If even one
// envelope could be visible, only a complete node packet can refine it.
fn visible_within(parent: Summary, frontier: &[Summary], assess: &impl Fn(u64, [f64; 6]) -> Option<Relevance>) -> bool {
    fn covered(key: u64, bounds: [f64; 6], frontier: &[Summary], assess: &impl Fn(u64, [f64; 6]) -> Option<Relevance>) -> bool {
        if frontier.iter().any(|s| s.key == key) {
            return true;
        }
        if !frontier.iter().any(|s| xy::contains(key, s.key)) {
            return false;
        }
        for child in xy::children(key) {
            let cell = xy::bounds(child);
            let mut b = bounds;
            if b[3] < cell[0] || b[0] >= cell[2] || b[4] < cell[1] || b[1] >= cell[3] {
                continue;
            }
            b[0] = b[0].max(cell[0]);
            b[1] = b[1].max(cell[1]);
            b[3] = b[3].min(cell[2]);
            b[4] = b[4].min(cell[3]);
            if assess(child, b).is_some() && !covered(child, b, frontier, assess) {
                return false;
            }
        }
        true
    }
    covered(parent.key, parent.bounds, frontier, assess)
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
