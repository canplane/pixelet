// @xpute/kit/collection/heap.ts

import type { primitive, u32 } from "../abi/word.ts";

// heap primitives over an external array (0-based).
//
// Element:
//   - key: primitive (priority key)
//   - val: payload
//
// Indexing (0-based):
//   parent(i) = (i - 1) >> 1
//   left(i)   = (i << 1) + 1
//   right(i)  = left(i) + 1
//
// Semantics:
//   - MinHeap: smaller key = higher priority (top is minimum key)
//   - MaxHeap: larger  key = higher priority (top is maximum key)
//
// Notes:
// - heapify(a, root, sz) assumes both child subtrees are already heaps.
// - build() is O(n) bottom-up heapify.
// - No defensive checks (caller upholds preconditions; keys are totally ordered).

export interface Element<K extends primitive, T> {
  key: K;
  val: T;
}

/** Whether a key goes before another: the smaller in a min-heap, the larger
 * in a max-heap. */
type Ahead = (x: primitive, y: primitive) => boolean;

const MIN: Ahead = (x, y) => x < y;
const MAX: Ahead = (x, y) => x > y;

/**
 * heapify(a, root, sz, ahead)
 * - restore the heap property of the subtree at `root`, within a[0..sz)
 * - assumes child subtrees already satisfy heap property.
 */
function heapify<K extends primitive, T>(
  a: Element<K, T>[],
  root: u32,
  sz: u32,
  ahead: Ahead,
): void {
  let i = root;
  const e = a[i];

  for (let child = (i << 1) + 1; child < sz; child = (i << 1) + 1) {
    const r = child + 1;
    if (r < sz && ahead(a[r].key, a[child].key)) child = r;

    if (!ahead(a[child].key, e.key)) break;
    a[i] = a[child];
    i = child;
  }

  a[i] = e;
}

/**
 * build(a, ahead)
 * - bottom-up heapify, O(n)
 */
function build<K extends primitive, T>(a: Element<K, T>[], ahead: Ahead): void {
  for (let i = (a.length >> 1) - 1; i >= 0; i--) {
    heapify(a, i, a.length, ahead);
  }
}

class Heap<K extends primitive, T> {
  readonly a: Element<K, T>[];
  private readonly _ahead: Ahead;

  // external storage (DOD-friendly): we operate on caller-owned array.
  constructor(a: Element<K, T>[], ahead: Ahead) {
    this.a = a;
    this._ahead = ahead;
  }

  len(): u32 {
    return this.a.length;
  }

  is_empty(): boolean {
    return this.a.length === 0;
  }

  top(): Element<K, T> | undefined {
    return this.a.length ? this.a[0] : undefined;
  }

  build(): void {
    build(this.a, this._ahead);
  }

  /**
   * push(e)
   * - hole sift-up
   */
  push(e: Element<K, T>): void {
    let i = this.a.length;
    this.a.push(e);

    for (; i > 0;) {
      const p = (i - 1) >> 1;
      if (!this._ahead(e.key, this.a[p].key)) break;
      this.a[i] = this.a[p];
      i = p;
    }

    this.a[i] = e;
  }

  /**
   * pop() -> Element | undefined
   * - removes and returns the top
   */
  pop(): Element<K, T> | undefined {
    const n = this.a.length;
    if (n === 0) return undefined;

    const out = this.a[0];

    if (n === 1) {
      this.a.pop();
      return out;
    }

    this.a[0] = this.a.pop()!;
    heapify(this.a, 0, n - 1, this._ahead);
    return out;
  }

  /**
   * replace_root(e)
   * - overwrite root and restore heap property
   * - requires non-empty heap
   */
  replace_root(e: Element<K, T>): void {
    this.a[0] = e;
    heapify(this.a, 0, this.a.length, this._ahead);
  }
}

export class MinHeap<K extends primitive, T> extends Heap<K, T> {
  constructor(a: Element<K, T>[]) {
    super(a, MIN);
  }

  static heapify<K extends primitive, T>(
    a: Element<K, T>[],
    root: u32,
    sz: u32,
  ): void {
    heapify(a, root, sz, MIN);
  }

  static build<K extends primitive, T>(a: Element<K, T>[]): void {
    build(a, MIN);
  }
}

export class MaxHeap<K extends primitive, T> extends Heap<K, T> {
  constructor(a: Element<K, T>[]) {
    super(a, MAX);
  }

  static heapify<K extends primitive, T>(
    a: Element<K, T>[],
    root: u32,
    sz: u32,
  ): void {
    heapify(a, root, sz, MAX);
  }

  static build<K extends primitive, T>(a: Element<K, T>[]): void {
    build(a, MAX);
  }
}

/**
 * heapsort(a)
 * - unstable, in-place, O(n log n)
 * - sorts ASC by key using MAX-heap (classic: pop max to the end)
 *
 * This mirrors the classic C heapsort structure:
 *   build (bottom-up heapify)
 *   for end=n-1..1: swap(a[0], a[end]); heapify(a, 0, end)
 */
export function heapsort<K extends primitive, T>(a: Element<K, T>[]): void {
  const n = a.length;
  if (n <= 1) return;

  MaxHeap.build(a);

  for (let end = n - 1; end > 0; end--) {
    const t = a[0];
    a[0] = a[end];
    a[end] = t;

    MaxHeap.heapify(a, 0, end);
  }
}
