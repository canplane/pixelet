// xpute-core/ds/treap_set.rs

//! treap_set.rs — Ordered set (BST key) with randomized balancing (heap priority)
//!
//! Data structure:
//!  - Binary Search Tree keyed by `comp(val, node.val)` (inorder is sorted order).
//!  - Heap property on `prio` (smaller prio closer to root) provides expected balance.
//!  - Each node stores subtree size `sz` for order statistics (kth()).
//!
//! Determinism:
//!  - If `seed` is provided:
//!      - priority RNG is deterministic for a given insertion order,
//!        so the treap shape is reproducible (same elements + same insert order).
//!  - If `seed` is omitted:
//!      - balancing RNG is implementation-defined (typically non-deterministic).
//!
//! Sampling primitives:
//!  - kth(k) returns the 0-based inorder element (order-statistic).
//!  - iter_from(start, limit) yields up to `limit` items starting from lower_bound(start)
//!    in inorder order, wrapping to the beginning if needed (circular window).
//!
//! Notes:
//!  - This is a set: duplicate inserts are ignored (comp(val, node.val) == 0).
//!  - All traversal order is defined purely by `comp`.
//!  - The generators are iterators here, each holding its stack.

use crate::kit::comp::Comp;
use crate::kit::rng::{make_rng, RNG};

struct Tree<T> {
    val: T,    // data (BST key)
    prio: u32, // priority (heap key)

    sz: u32, // subtree size (for unbiased sampling)
    left: Option<Box<Tree<T>>>,
    right: Option<Box<Tree<T>>>,
}

pub struct TreapSet<T> {
    root: Option<Box<Tree<T>>>,

    rng: RNG,

    comp: Comp<T>,
}

impl<T> TreapSet<T> {
    pub fn new(comp: Comp<T>, seed: Option<u64>) -> TreapSet<T> {
        // Deterministic treap balancing:
        //  - same seed + same insertion order => same shape
        //  - if seed is omitted, RNG behavior is implementation-defined (often non-deterministic)
        TreapSet {
            root: None,
            rng: make_rng(seed),
            comp,
        } // balancing RNG
    }

    pub fn size(&self) -> u32 {
        Self::_size(&self.root)
    }

    fn _size(node: &Option<Box<Tree<T>>>) -> u32 {
        node.as_ref().map_or(0, |n| n.sz)
    }

    /// 0-based kth element in inorder
    pub fn kth(&self, k: u32) -> Option<&T> {
        let mut k = k;
        let mut node = self.root.as_deref();
        while let Some(n) = node {
            let ls = Self::_size(&n.left);
            if k < ls {
                node = n.left.as_deref();
            } else if k == ls {
                return Some(&n.val);
            } else {
                k = k.wrapping_sub(ls + 1);
                node = n.right.as_deref();
            }
        }
        // unreachable if sizes are correct
        None
    }

    /// [Search] O(log N) iterative search
    pub fn has(&self, val: &T) -> bool {
        let mut n = self.root.as_deref();
        while let Some(node) = n {
            let c = (self.comp)(val, &node.val);
            if c == 0 {
                return true;
            }
            n = if c < 0 { node.left.as_deref() } else { node.right.as_deref() };
        }
        false
    }

    pub fn insert(&mut self, val: T) {
        let root = self.root.take();
        self.root = Some(self._insert(root, val));
    }

    pub fn delete(&mut self, val: &T) {
        let root = self.root.take();
        self.root = self._delete(root, val);
    }

    /// [Traverse] Stack-based non-recursive inorder
    pub fn iter(&self) -> Inorder<'_, T> {
        Inorder {
            st: Vec::new(),
            cur: self.root.as_deref(),
            left: u32::MAX,
        }
    }

    pub fn iter_prefix(&self, limit: i64) -> Inorder<'_, T> {
        let Some(root) = self.root.as_deref() else {
            return Inorder { st: Vec::new(), cur: None, left: 0 };
        };
        if limit <= 0 {
            return Inorder { st: Vec::new(), cur: None, left: 0 };
        }
        let out_limit = (limit.min(root.sz as i64)) as u32;
        Inorder {
            st: Vec::new(),
            cur: Some(root),
            left: out_limit,
        }
    }

    /// [Traverse] inorder from lower_bound(start), wrapping to begin if needed.
    ///
    /// Intended use for unbiased-ish windows:
    ///  - choose `start` via kth(rand % size), then iter_from(start, limit).
    pub fn iter_from(&self, start: &T, limit: i64) -> From<'_, T> {
        let Some(root) = self.root.as_deref() else {
            return From {
                st: Vec::new(),
                root: None,
                left: 0,
                wrapped: true,
            };
        };
        if limit <= 0 {
            return From {
                st: Vec::new(),
                root: None,
                left: 0,
                wrapped: true,
            };
        }
        let out_limit = (limit.min(root.sz as i64)) as u32;

        // build stack to lower_bound(start)
        let mut st: Vec<&Tree<T>> = Vec::new();
        let mut cur = Some(root);
        while let Some(n) = cur {
            let c = (self.comp)(start, &n.val);
            if c <= 0 {
                st.push(n);
                cur = n.left.as_deref();
            } else {
                cur = n.right.as_deref();
            }
        }
        From {
            st,
            root: Some(root),
            left: out_limit,
            wrapped: false,
        }
    }

    fn _rot_r(mut y: Box<Tree<T>>) -> Box<Tree<T>> {
        let mut x = y.left.take().unwrap();
        y.left = x.right.take();
        Self::_pull(&mut y);
        x.right = Some(y);
        Self::_pull(&mut x);
        x
    }

    fn _rot_l(mut x: Box<Tree<T>>) -> Box<Tree<T>> {
        let mut y = x.right.take().unwrap();
        x.right = y.left.take();
        Self::_pull(&mut x);
        y.left = Some(x);
        Self::_pull(&mut y);
        y
    }

    fn _pull(node: &mut Tree<T>) {
        node.sz = 1 + Self::_size(&node.left) + Self::_size(&node.right);
    }

    fn _insert(&mut self, node: Option<Box<Tree<T>>>, val: T) -> Box<Tree<T>> {
        let Some(mut node) = node else {
            let prio = (self.rng)();
            return Box::new(Tree {
                val,
                prio,
                sz: 1,
                left: None,
                right: None,
            });
        };

        let c = (self.comp)(&val, &node.val);
        if c == 0 {
            return node; // set semantics
        }

        if c < 0 {
            let left = node.left.take();
            node.left = Some(self._insert(left, val));
            if node.left.as_ref().unwrap().prio < node.prio {
                node = Self::_rot_r(node);
            }
        } else {
            let right = node.right.take();
            node.right = Some(self._insert(right, val));
            if node.right.as_ref().unwrap().prio < node.prio {
                node = Self::_rot_l(node);
            }
        }
        Self::_pull(&mut node);
        node
    }

    fn _delete(&mut self, node: Option<Box<Tree<T>>>, val: &T) -> Option<Box<Tree<T>>> {
        let mut node = node?;

        let c = (self.comp)(val, &node.val);
        if c < 0 {
            let left = node.left.take();
            node.left = self._delete(left, val);
            Self::_pull(&mut node);
            return Some(node);
        }
        if c > 0 {
            let right = node.right.take();
            node.right = self._delete(right, val);
            Self::_pull(&mut node);
            return Some(node);
        }

        // found: rotate down until leaf-ish
        if node.left.is_none() {
            return node.right.take();
        }
        if node.right.is_none() {
            return node.left.take();
        }

        if node.left.as_ref().unwrap().prio < node.right.as_ref().unwrap().prio {
            node = Self::_rot_r(node);
            let right = node.right.take();
            node.right = self._delete(right, val);
        } else {
            node = Self::_rot_l(node);
            let left = node.left.take();
            node.left = self._delete(left, val);
        }
        Self::_pull(&mut node);
        Some(node)
    }
}

/// `iter` and `iter_prefix`: the stack-based inorder walk, `left` items to go.
pub struct Inorder<'a, T> {
    st: Vec<&'a Tree<T>>,
    cur: Option<&'a Tree<T>>,
    left: u32,
}

impl<'a, T> Iterator for Inorder<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        if self.left == 0 {
            return None;
        }
        while let Some(c) = self.cur {
            self.st.push(c);
            self.cur = c.left.as_deref();
        }
        let n = self.st.pop()?;
        self.cur = n.right.as_deref();
        self.left -= 1;
        Some(&n.val)
    }
}

/// `iter_from`: from lower_bound(start), then from the beginning.
pub struct From<'a, T> {
    st: Vec<&'a Tree<T>>,
    root: Option<&'a Tree<T>>,
    left: u32,
    wrapped: bool,
}

impl<'a, T> From<'a, T> {
    // iterator step: pop next, then go right->left chain
    fn step(st: &mut Vec<&'a Tree<T>>) -> &'a T {
        let n = st.pop().unwrap();
        let mut r = n.right.as_deref();
        while let Some(node) = r {
            st.push(node);
            r = node.left.as_deref();
        }
        &n.val
    }
}

impl<'a, T> Iterator for From<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        if self.left == 0 {
            return None;
        }
        if self.st.is_empty() {
            if self.wrapped {
                return None;
            }
            // wrap: from begin
            self.wrapped = true;
            let mut node = self.root;
            while let Some(n) = node {
                self.st.push(n);
                node = n.left.as_deref();
            }
            if self.st.is_empty() {
                return None;
            }
        }
        self.left -= 1;
        Some(Self::step(&mut self.st))
    }
}

#[cfg(test)]
#[path = "treap_set.test.rs"]
mod test;
