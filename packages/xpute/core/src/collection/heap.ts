// @xpute/core/collection/heap.ts
// heap primitives over an external array (0-based).
//
// Element:
//   - key: K (priority key: anything `<` orders)
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
// - heapify(root, size) assumes both child subtrees are already heaps.
// - build() is O(n) bottom-up heapify.
// - No defensive checks (caller upholds preconditions; keys are totally ordered).

import type { primitive, u32 } from "../abi/word.ts";

export interface Element<K extends primitive, T> {
  key: K;
  val: T;
}

export class MinHeap<K extends primitive, T> {
  // external storage (DOD-friendly): we operate on caller-owned array.
  constructor(readonly a: Element<K, T>[]) {}

  size(): u32 {
    return this.a.length;
  }

  empty(): boolean {
    return this.a.length === 0;
  }

  top(): Element<K, T> | undefined {
    return this.a.length ? this.a[0] : undefined;
  }

  /**
   * heapify(a, root, size)
   * - restore MIN-heap property in a[0..size)
   * - assumes child subtrees already satisfy heap property.
   */
  static heapify<K extends primitive, T>(a: Element<K, T>[], root: u32, size: u32): void {
    let i = root;
    const e = a[i];

    for (let child = (i << 1) + 1; child < size; child = (i << 1) + 1) {
      const r = child + 1;
      if (r < size && a[r].key < a[child].key) child = r;

      if (a[child].key < e.key) {
        a[i] = a[child];
        i = child;
      } else {
        break;
      }
    }

    a[i] = e;
  }

  /**
   * build(a)
   * - bottom-up heapify, O(n)
   */
  static build<K extends primitive, T>(a: Element<K, T>[]): void {
    for (let i = (a.length >> 1) - 1; i >= 0; i--) MinHeap.heapify(a, i, a.length);
  }

  build(): void {
    MinHeap.build(this.a);
  }

  /**
   * push(e)
   * - hole sift-up (MIN)
   */
  push(e: Element<K, T>): void {
    let i = this.a.length;
    this.a.push(e);

    for (; i > 0;) {
      const p = (i - 1) >> 1;
      if (this.a[p].key <= e.key) break;
      this.a[i] = this.a[p];
      i = p;
    }

    this.a[i] = e;
  }

  /**
   * pop() -> Element | undefined
   * - removes and returns root (minimum key)
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
    MinHeap.heapify(this.a, 0, n - 1);
    return out;
  }

  /**
   * replace_root(e)
   * - overwrite root and restore heap property
   * - requires non-empty heap
   */
  replace_root(e: Element<K, T>): void {
    this.a[0] = e;
    MinHeap.heapify(this.a, 0, this.a.length);
  }
}

export class MaxHeap<K extends primitive, T> {
  constructor(readonly a: Element<K, T>[]) {}

  size(): u32 {
    return this.a.length;
  }

  empty(): boolean {
    return this.a.length === 0;
  }

  top(): Element<K, T> | undefined {
    return this.a.length ? this.a[0] : undefined;
  }

  /**
   * heapify(a, root, size)
   * - restore MAX-heap property in a[0..size)
   * - assumes child subtrees already satisfy heap property.
   */
  static heapify<K extends primitive, T>(a: Element<K, T>[], root: u32, size: u32): void {
    let i = root;
    const e = a[i];

    for (let child = (i << 1) + 1; child < size; child = (i << 1) + 1) {
      const r = child + 1;
      if (r < size && a[r].key > a[child].key) child = r;

      if (a[child].key > e.key) {
        a[i] = a[child];
        i = child;
      } else {
        break;
      }
    }

    a[i] = e;
  }

  static build<K extends primitive, T>(a: Element<K, T>[]): void {
    for (let i = (a.length >> 1) - 1; i >= 0; i--) MaxHeap.heapify(a, i, a.length);
  }

  build(): void {
    MaxHeap.build(this.a);
  }

  /**
   * push(e)
   * - hole sift-up (MAX)
   */
  push(e: Element<K, T>): void {
    let i = this.a.length;
    this.a.push(e);

    for (; i > 0;) {
      const p = (i - 1) >> 1;
      if (this.a[p].key >= e.key) break;
      this.a[i] = this.a[p];
      i = p;
    }

    this.a[i] = e;
  }

  /**
   * pop() -> Element | undefined
   * - removes and returns root (maximum key)
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
    MaxHeap.heapify(this.a, 0, n - 1);
    return out;
  }

  /**
   * replace_root(e)
   * - overwrite root and restore heap property
   * - requires non-empty heap
   */
  replace_root(e: Element<K, T>): void {
    this.a[0] = e;
    MaxHeap.heapify(this.a, 0, this.a.length);
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
