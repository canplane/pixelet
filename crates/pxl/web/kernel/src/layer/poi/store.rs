// pxl-kernel/layer/poi/store.rs

use crate::global::Global;
use crate::io::{demand::Category, policy, sched::declare};
use crate::shell::config::env::APP_ENV;
use crate::shell::setting::store::settings_store;
use crate::world::change::request_world_tick;
use pxl_spec::poi::{wire, Node, Summary};

pub const CANDIDATES: usize = 8;
pub const RESIDENT_BYTES: usize = 4096;

#[derive(Default)]
pub struct Store {
    nodes: Vec<Node>,
    pub cut: Vec<Summary>,
    wanted: Vec<u64>,
}

impl Store {
    pub fn bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<Node>() + self.nodes.iter().map(|n| n.children.capacity() * std::mem::size_of::<Summary>()).sum::<usize>()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn adopt(&mut self, subject: u64, node: Node, budget: usize) -> Result<(), &'static str> {
        wire::validate(&node)?;
        if !self.wanted.contains(&subject) {
            return Ok(());
        }
        if subject == 0 {
            if !self.nodes.is_empty() {
                return Err("root already resident");
            }
        } else {
            let root = self.nodes.first().ok_or("root missing")?;
            let expected = self.nodes.iter().flat_map(|n| &n.children).find(|s| s.key == subject).ok_or("unrequested refinement")?;
            if node.revision != root.revision || node.summary != *expected {
                return Err("refinement snapshot mismatch");
            }
        }
        let slot = usize::from(self.nodes.len() == self.nodes.capacity()) * std::mem::size_of::<Node>();
        let bytes = slot + node.children.capacity() * std::mem::size_of::<Summary>();
        if self.bytes() + bytes > budget {
            return Err("summary residency budget");
        }
        self.nodes.reserve_exact(1);
        self.nodes.push(node);
        self.wanted.retain(|&key| key != subject);
        Ok(())
    }

    /// A cut is a partition of the root's membership. A refinement replaces
    /// one summary only when its entire set of children fits. Missing data or
    /// memory leaves that summary in the cut; neither can erase its members.
    pub fn select(&mut self, eye: [f64; 3], max_candidates: usize, budget: usize) {
        self.cut.clear();
        self.wanted.clear();
        let Some(root) = self.nodes.first() else {
            self.wanted.push(0);
            return;
        };
        let mut keep = vec![root.summary.key];
        if root.summary.count == 0 {
            return;
        }
        self.cut.push(root.summary);
        let mut i = 0;
        while i < self.cut.len() {
            let s = self.cut[i];
            let span = (s.bounds[3] - s.bounds[0]).max(s.bounds[4] - s.bounds[1]);
            let distance = (0..3).map(|a| (eye[a] - eye[a].clamp(s.bounds[a], s.bounds[a + 3])).powi(2)).sum::<f64>().sqrt();
            // The threshold is a presentation resolution, not a population
            // limit. Coincident entities remain a summary even at the leaf.
            if !s.refinable || distance > span.max(64.0) * 3.0 {
                i += 1;
                continue;
            }
            if let Some(node) = self.nodes.iter().find(|n| n.summary.key == s.key) {
                if self.cut.len() - 1 + node.children.len() <= max_candidates.max(1) {
                    keep.push(s.key);
                    self.cut.splice(i..=i, node.children.iter().copied());
                    continue;
                }
            } else {
                // Reserve for the largest response before requesting it, so
                // simultaneous arrivals cannot consume one another's room.
                let reservation = std::mem::size_of::<Node>() + pxl_spec::poi::xy::CHILDREN * std::mem::size_of::<Summary>();
                if self.bytes() + (self.wanted.len() + 1) * reservation <= budget {
                    self.wanted.push(s.key);
                }
            }
            i += 1;
        }
        // Nodes contain immutable child summaries. Keeping the traversed
        // parents preserves a coarse fallback without retaining raw detail.
        self.nodes.retain(|n| keep.contains(&n.summary.key) || self.cut.iter().any(|s| s.key == n.summary.key));
    }

    pub fn wanted(&self) -> &[u64] {
        &self.wanted
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
    let wants: Vec<_> = store().wanted().iter().map(|&key| (key, policy::poi(0.0))).collect();
    declare(Category::Poi, &wants);
}

pub fn adopt(subject: u64, bytes: &[u8]) -> Result<(), &'static str> {
    let node = wire::decode(bytes)?;
    store().adopt(subject, node, RESIDENT_BYTES)?;
    demand();
    request_world_tick();
    Ok(())
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
