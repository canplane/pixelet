// @pixelet/shared/ds/treap_set.ts

/**
 * treap_set.ts — Ordered set (BST key) with randomized balancing (heap priority)
 *
 * Data structure:
 *  - Binary Search Tree keyed by `comp(val, node.val)` (inorder is sorted order).
 *  - Heap property on `prio` (smaller prio closer to root) provides expected balance.
 *  - Each node stores subtree size `sz` for order statistics (kth()).
 *
 * Determinism:
 *  - If `seed` is provided:
 *      - priority RNG is deterministic for a given insertion order,
 *        so the treap shape is reproducible (same elements + same insert order).
 *  - If `seed` is omitted:
 *      - balancing RNG is implementation-defined (typically non-deterministic).
 *
 * Sampling primitives:
 *  - kth(k) returns the 0-based inorder element (order-statistic).
 *  - iter_from(start, limit) yields up to `limit` items starting from lower_bound(start)
 *    in inorder order, wrapping to the beginning if needed (circular window).
 *
 * Notes:
 *  - This is a set: duplicate inserts are ignored (comp(val, node.val) == 0).
 *  - All traversal order is defined purely by `comp`.
 */

import { U32, type u32 } from "../kit/word.ts";
import type { Comp } from "@pixelet/shared/kit/comp.ts";
import { make_rng, type RNG } from "@pixelet/shared/kit/rng.ts";

export type Key = string | number | bigint;

type Tree<T> = {
  val: T; // 데이터 (BST Key)
  prio: u32; // 우선순위 (Heap Key)

  sz: u32; // subtree size (for unbiased sampling)
  left: Tree<T> | null;
  right: Tree<T> | null;
};

export class TreapSet<T> {
  private root: Tree<T> | null = null;

  private readonly rng: RNG;

  constructor(
    private readonly comp: Comp<T>,
    private readonly seed?: bigint,
  ) {
    // Deterministic treap balancing:
    //  - same seed + same insertion order => same shape
    //  - if seed is omitted, RNG behavior is implementation-defined (often non-deterministic)
    this.rng = make_rng(this.seed); // balancing RNG
  }

  size(): u32 {
    return this._size(this.root);
  }

  private _size(node: Tree<T> | null): u32 {
    return node ? node.sz : 0;
  }

  /** 0-based kth element in inorder */
  kth(k: u32): T | undefined {
    let node: Tree<T> | null = this.root;
    while (node) {
      const ls: u32 = this._size(node.left);
      if (k < ls) {
        node = node.left;
      } else if (k === ls) {
        return node.val;
      } else {
        k = U32(k - ls - 1);
        node = node.right;
      }
    }
    // unreachable if sizes are correct
    return undefined;
  }

  /** [Search] O(log N) iterative search */
  has(val: T): boolean {
    let n: Tree<T> | null = this.root;
    while (n) {
      const c = this.comp(val, n.val);
      if (c === 0) return true;
      n = c < 0 ? n.left : n.right;
    }
    return false;
  }

  insert(val: T): void {
    this.root = this._insert(this.root, val);
  }

  delete(val: T): void {
    this.root = this._delete(this.root, val);
  }

  /** [Traverse] Stack-based non-recursive inorder */
  *iter(): IterableIterator<T> {
    // stack-based inorder
    const st: Array<Tree<T>> = [];
    let cur: Tree<T> | null = this.root;
    while (cur || st.length > 0) {
      while (cur) {
        st.push(cur);
        cur = cur.left;
      }
      cur = st.pop()!;
      yield cur.val;
      cur = cur.right;
    }
  }

  *iter_prefix(limit: number): IterableIterator<T> {
    if (limit <= 0 || !this.root) return;
    const out_limit: u32 = U32(Math.min(limit, this.root.sz));
    if (out_limit <= 0) return;
    let out: u32 = 0;

    const st: Array<Tree<T>> = [];
    let cur: Tree<T> | null = this.root;
    while ((cur || st.length > 0) && out < out_limit) {
      while (cur) {
        st.push(cur);
        cur = cur.left;
      }
      cur = st.pop()!;
      yield cur.val;
      out++;
      cur = cur.right;
    }
  }

  /**
   * [Traverse] inorder from lower_bound(start), wrapping to begin if needed.
   *
   * Intended use for unbiased-ish windows:
   *  - choose `start` via kth(rand % size), then iter_from(start, limit).
   */
  *iter_from(start: T, limit: number): IterableIterator<T> {
    if (limit <= 0 || !this.root) return;
    const out_limit: u32 = U32(Math.min(limit, this.root.sz));
    if (out_limit <= 0) return;
    let out: u32 = 0;

    const push_left = (node: Tree<T> | null, st: Array<Tree<T>>) => {
      while (node) {
        st.push(node);
        node = node.left;
      }
    };

    // build stack to lower_bound(start)
    const st: Array<Tree<T>> = [];
    let cur: Tree<T> | null = this.root;
    while (cur) {
      const c = this.comp(start, cur.val);
      if (c <= 0) {
        st.push(cur);
        cur = cur.left;
      } else {
        cur = cur.right;
      }
    }

    // iterator step: pop next, then go right->left chain
    const step = (st2: Array<Tree<T>>): T => {
      const n = st2.pop()!;
      let r: Tree<T> | null = n.right;
      while (r) {
        st2.push(r);
        r = r.left;
      }
      return n.val;
    };

    // phase 1: from lower_bound(start)
    while (st.length && out < out_limit) {
      yield step(st);
      out++;
    }

    // wrap: from begin
    if (out >= out_limit) return;
    const st2: Array<Tree<T>> = [];
    push_left(this.root, st2);
    while (st2.length && out < out_limit) {
      yield step(st2);
      out++;
    }
  }

  private _rotR(y: Tree<T>): Tree<T> {
    const x = y.left!;
    y.left = x.right;
    x.right = y;
    this._pull(y);
    this._pull(x);
    return x;
  }

  private _rotL(x: Tree<T>): Tree<T> {
    const y = x.right!;
    x.right = y.left;
    y.left = x;
    this._pull(x);
    this._pull(y);
    return y;
  }

  private _pull(node: Tree<T>): void {
    node.sz = U32(1 + this._size(node.left) + this._size(node.right));
  }

  private _insert(node: Tree<T> | null, val: T): Tree<T> {
    if (!node) {
      const prio: u32 = this.rng();
      return { val, prio, sz: 1, left: null, right: null };
    }

    const c = this.comp(val, node.val);
    if (c === 0) return node; // set semantics

    if (c < 0) {
      node.left = this._insert(node.left, val);
      if (node.left!.prio < node.prio) node = this._rotR(node);
    } else {
      node.right = this._insert(node.right, val);
      if (node.right!.prio < node.prio) node = this._rotL(node);
    }
    this._pull(node);
    return node;
  }

  private _delete(node: Tree<T> | null, val: T): Tree<T> | null {
    if (!node) return null;

    const c = this.comp(val, node.val);
    if (c < 0) {
      node.left = this._delete(node.left, val);
      this._pull(node);
      return node;
    }
    if (c > 0) {
      node.right = this._delete(node.right, val);
      this._pull(node);
      return node;
    }

    // found: rotate down until leaf-ish
    if (!node.left) return node.right;
    if (!node.right) return node.left;

    if (node.left.prio < node.right.prio) {
      node = this._rotR(node);
      node.right = this._delete(node.right, val);
    } else {
      node = this._rotL(node);
      node.left = this._delete(node.left, val);
    }
    this._pull(node);
    return node;
  }
}
