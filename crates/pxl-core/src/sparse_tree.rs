//! A 2³-ary sparse tree over two lanes — @pxl/shared/spec/space/sparse_tree/mem.ts
//! with the shape the object store gives it: three axes and a 16-bit leaf.
//!
//! The structural unit is a block of eight child slots, not a node: `heap`
//! holds one tagged word a slot — the top bit set names another block's
//! base, clear makes the slot a leaf whose value is `data`'s at the same
//! slot — and the root is the block at base 0, never a scalar. A key's
//! bits are consumed one per axis per level from the top, so a tree of
//! depth `dep` tells apart positions within one `2^dep` span. Kept
//! slot for slot with the TypeScript so a walk visits the same leaves in
//! the same order, which is what makes an entry lane the same here as there.

const WIDTH: usize = 8;
const NODE_MSB: u32 = 0x8000_0000;
const BLOCK_MASK: u32 = 0x7fff_ffff;

pub struct Tree {
    heap: Vec<u32>,
    data: Vec<u16>,
    /// Levels between a leaf and the root block.
    dep: u32,
}

/// A position in the tree's lattice, one word an axis; the low `dep` bits
/// of each are the key.
pub type Key = [u32; 3];

impl Tree {
    pub fn new(dep: u32) -> Tree {
        assert!(dep >= 1, "a tree needs a level below its root");
        let mut tree = Tree {
            heap: Vec::with_capacity(64),
            data: Vec::with_capacity(64),
            dep,
        };
        tree.alloc_block();
        tree
    }

    pub fn dep(&self) -> u32 {
        self.dep
    }

    /// Slots in use.
    pub fn head(&self) -> usize {
        self.heap.len()
    }

    /// One zeroed block; its base.
    fn alloc_block(&mut self) -> u32 {
        let base = self.heap.len();
        self.heap.resize(base + WIDTH, 0);
        self.data.resize(base + WIDTH, 0);
        base as u32
    }

    fn child(key: Key, u: u32) -> usize {
        (((key[0] >> u) & 1) | (((key[1] >> u) & 1) << 1) | (((key[2] >> u) & 1) << 2)) as usize
    }

    /// The leaf `key` lands in at the finest level, or the coarser leaf
    /// that covers it.
    pub fn get(&self, key: Key) -> u16 {
        let mut base = 0usize;
        for u in (0..self.dep).rev() {
            let slot = base + Self::child(key, u);
            let node = self.heap[slot];
            if u == 0 || node & NODE_MSB == 0 {
                return self.data[slot];
            }
            base = (node & BLOCK_MASK) as usize;
        }
        self.data[base]
    }

    /// Writes `val` as one leaf at `target_u` levels above the finest,
    /// splitting only the blocks on the way down that are not already
    /// `val`. Reaching the target claims the slot whole: whatever finer
    /// structure hung under it is orphaned until the next compact.
    pub fn set(&mut self, key: Key, val: u16, target_u: u32) {
        let mut base = 0usize;
        for u in (target_u..self.dep).rev() {
            let slot = base + Self::child(key, u);
            if u == target_u {
                if self.heap[slot] & NODE_MSB == 0 && self.data[slot] == val {
                    return;
                }
                self.heap[slot] = 0;
                self.data[slot] = val;
                return;
            }
            let node = self.heap[slot];
            if node & NODE_MSB != 0 {
                base = (node & BLOCK_MASK) as usize;
                continue;
            }
            if self.data[slot] == val {
                return;
            }
            let next = self.alloc_block() as usize;
            let was = self.data[slot];
            self.data[next..next + WIDTH].fill(was);
            self.heap[slot] = next as u32 | NODE_MSB;
            base = next;
        }
    }

    /// Visits every maximal uniform leaf block: its corner in the finest
    /// lattice, its level above the finest (`1 << u` is its span), its
    /// value. A root whose eight slots are one leaf value is reported as
    /// one block at `dep`, as the TypeScript does.
    pub fn walk(&self, mut visit: impl FnMut(u32, u32, u32, u32, u16)) {
        let first = self.data[0];
        let uniform = (0..WIDTH).all(|c| self.heap[c] & NODE_MSB == 0 && self.data[c] == first);
        if uniform {
            visit(0, 0, 0, self.dep, first);
            return;
        }
        self.walk_block(0, self.dep - 1, 0, 0, 0, &mut visit);
    }

    fn walk_block(&self, base: usize, u: u32, x: u32, y: u32, z: u32, visit: &mut impl FnMut(u32, u32, u32, u32, u16)) {
        for c in 0..WIDTH {
            let slot = base + c;
            let node = self.heap[slot];
            let cx = x | (((c & 1) as u32) << u);
            let cy = y | ((((c >> 1) & 1) as u32) << u);
            let cz = z | ((((c >> 2) & 1) as u32) << u);
            if node & NODE_MSB != 0 {
                self.walk_block((node & BLOCK_MASK) as usize, u - 1, cx, cy, cz, visit);
            } else {
                visit(cx, cy, cz, u, self.data[slot]);
            }
        }
    }

    /// Whether every leaf reads zero.
    pub fn is_empty(&self) -> bool {
        let mut empty = true;
        self.walk(|_, _, _, _, v| {
            if v != 0 {
                empty = false;
            }
        });
        empty
    }

    /// Collapses every block whose leaves agree into one leaf, then
    /// rewrites the tree depth first so orphaned blocks are dropped and
    /// the live ones are in walk order.
    pub fn compact(&mut self) {
        self.prune(0, self.dep);
        let mut out = Tree::new(self.dep);
        self.relocate(0, 0, &mut out);
        *self = out;
    }

    /// The one value the block at `base` collapses to, if it does.
    fn prune(&mut self, base: usize, dep: u32) -> Option<u16> {
        if dep == 0 {
            return None;
        }
        let mut uniform = true;
        let mut first: Option<u16> = None;
        for c in 0..WIDTH {
            let slot = base + c;
            let node = self.heap[slot];
            let mut val = if node & NODE_MSB != 0 { None } else { Some(self.data[slot]) };
            if node & NODE_MSB != 0 {
                match self.prune((node & BLOCK_MASK) as usize, dep - 1) {
                    Some(v) => {
                        self.heap[slot] = 0;
                        self.data[slot] = v;
                        val = Some(v);
                    }
                    None => uniform = false,
                }
            }
            match (first, val) {
                (None, _) => first = val,
                (Some(f), Some(v)) if f == v => {}
                _ => uniform = false,
            }
        }
        if uniform {
            first
        } else {
            None
        }
    }

    fn relocate(&self, old_base: usize, new_base: usize, out: &mut Tree) {
        for c in 0..WIDTH {
            let node = self.heap[old_base + c];
            if node & NODE_MSB != 0 {
                let next = out.alloc_block() as usize;
                out.heap[new_base + c] = next as u32 | NODE_MSB;
                self.relocate((node & BLOCK_MASK) as usize, next, out);
            } else {
                out.data[new_base + c] = self.data[old_base + c];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(tree: &Tree) -> Vec<(u32, u32, u32, u32, u16)> {
        let mut out = Vec::new();
        tree.walk(|x, y, z, u, v| out.push((x, y, z, u, v)));
        out
    }

    #[test]
    fn a_fresh_tree_is_one_zero_block() {
        let tree = Tree::new(3);
        assert_eq!(leaves(&tree), vec![(0, 0, 0, 3, 0)]);
        assert!(tree.is_empty());
    }

    #[test]
    fn a_leaf_is_read_back_and_walked_at_its_level() {
        let mut tree = Tree::new(3);
        tree.set([5, 2, 7], 9, 0);
        assert_eq!(tree.get([5, 2, 7]), 9);
        assert_eq!(tree.get([4, 2, 7]), 0);
        tree.set([0, 4, 0], 3, 2);
        assert_eq!(tree.get([3, 7, 3]), 3, "a coarse leaf covers its span");
        assert_eq!(tree.get([4, 7, 3]), 0);
        let found = leaves(&tree);
        assert!(found.contains(&(5, 2, 7, 0, 9)));
        assert!(found.contains(&(0, 4, 0, 2, 3)));
    }

    #[test]
    fn compact_collapses_agreeing_blocks_and_drops_orphans() {
        let mut tree = Tree::new(3);
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    tree.set([x, y, z], 4, 0);
                }
            }
        }
        let before = tree.head();
        tree.compact();
        assert!(tree.head() < before);
        assert!(leaves(&tree).contains(&(0, 0, 0, 1, 4)));
        // Claiming a root slot whole orphans the block under it.
        tree.set([0, 0, 0], 0, 2);
        assert_eq!(tree.head(), WIDTH * 2, "the claim leaves the orphaned block behind");
        tree.compact();
        assert!(tree.is_empty());
        assert_eq!(tree.head(), WIDTH);
    }
}
