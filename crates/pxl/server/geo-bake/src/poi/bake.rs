// pxl-geo-bake/poi/bake.rs

use pxl_spec::poi::{xy, Entity, Node, Summary};
use std::collections::BTreeMap;

/// Only summaries are accumulated. Coincident entities increase counts,
/// never the number of entries or the size of a published node.
pub struct Hierarchy {
    root: u64,
    revision: u64,
    summaries: BTreeMap<u64, Summary>,
    last: Option<Entity>,
}

impl Hierarchy {
    pub fn new(root: u64, revision: u64) -> Result<Self, &'static str> {
        let depth = xy::depth(root).ok_or("invalid root")?;
        if depth != xy::ROOT_DEPTH || revision == 0 {
            return Err("root must have 15 digits and revision must be nonzero");
        }
        Ok(Self {
            root,
            revision,
            summaries: BTreeMap::new(),
            last: None,
        })
    }

    /// Sorted, exact identities let a streaming bake deduplicate without
    /// keeping a raw set in memory. Conflicting observations require a source
    /// policy; silently choosing one would manufacture canonical ownership.
    pub fn add(&mut self, entity: Entity) -> Result<bool, &'static str> {
        if let Some(last) = &self.last {
            if entity.id < last.id {
                return Err("entities must be sorted by identity");
            }
            if entity.id == last.id {
                return if entity == *last { Ok(false) } else { Err("conflicting entity identity") };
            }
        }
        let key = xy::key_at(entity.position, xy::MAX_DEPTH).ok_or("invalid canonical XYZ")?;
        if entity.id.is_empty() || !xy::contains(self.root, key) {
            return Err("entity outside coverage or without identity");
        }
        if self.summaries.get(&self.root).is_some_and(|s| s.count == u64::MAX) {
            return Err("count overflow");
        }
        self.add_at(self.root, &entity);
        self.last = Some(entity);
        Ok(true)
    }

    fn add_at(&mut self, key: u64, entity: &Entity) {
        let Some(old) = self.summaries.get(&key).copied() else {
            let [x, y, z] = entity.position;
            self.summaries.insert(
                key,
                Summary {
                    key,
                    count: 1,
                    bounds: [x, y, z, x, y, z],
                    refinable: false,
                },
            );
            return;
        };
        let depth = xy::depth(key).unwrap();
        let split = depth < xy::MAX_DEPTH && (old.refinable || old.bounds[0] != entity.position[0] || old.bounds[1] != entity.position[1]);
        let current = self.summaries.get_mut(&key).unwrap();
        current.count += 1;
        current.refinable = split;
        for i in 0..3 {
            current.bounds[i] = current.bounds[i].min(entity.position[i]);
            current.bounds[i + 3] = current.bounds[i + 3].max(entity.position[i]);
        }
        if split {
            if !old.refinable {
                // Until this split the whole prior population shared XY.
                // Move its aggregate, not its raw entities, into the child.
                let child = xy::key_at([old.bounds[0], old.bounds[1], old.bounds[2]], depth + 1).unwrap();
                self.summaries.insert(child, Summary { key: child, ..old });
            }
            let child = xy::key_at(entity.position, depth + 1).unwrap();
            self.add_at(child, entity);
        }
    }

    pub fn finish(mut self) -> Vec<Node> {
        if self.summaries.is_empty() {
            let [x, y, _, _] = xy::bounds(self.root);
            self.summaries.insert(
                self.root,
                Summary {
                    key: self.root,
                    count: 0,
                    bounds: [x, y, 0.0, x, y, 0.0],
                    refinable: false,
                },
            );
        }
        let mut nodes = Vec::new();
        self.emit(self.root, &mut nodes);
        nodes
    }

    fn summary(&self, key: u64) -> Option<Summary> {
        self.summaries.get(&key).map(|s| Summary {
            // XY transport cannot separate a vertical stack. Its full XYZ
            // bounds and count survive; entity access belongs to detail.
            refinable: s.refinable && (s.bounds[0] < s.bounds[3] || s.bounds[1] < s.bounds[4]),
            ..*s
        })
    }

    fn emit(&self, key: u64, nodes: &mut Vec<Node>) {
        let summary = self.summary(key).unwrap();
        let children: Vec<_> = if summary.refinable {
            xy::children(key).into_iter().filter_map(|c| self.summary(c)).collect()
        } else {
            Vec::new()
        };
        let descendants: Vec<_> = children.iter().map(|c| c.key).collect();
        nodes.push(Node {
            revision: self.revision,
            summary,
            children,
        });
        for child in descendants {
            self.emit(child, nodes);
        }
    }
}

#[cfg(test)]
#[path = "bake.test.rs"]
mod test;
