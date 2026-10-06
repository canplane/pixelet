// @xpute/core/collection/heap.ts
// Binary min/max heaps over a caller-owned array, with no defensive checks.

import type { primitive, u32 } from "../abi/word.ts";

export interface Element<K extends primitive, T> {
  key: K;
  val: T;
}

export class MinHeap<K extends primitive, T> {
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

  /** Assumes both child subtrees are already heaps. */
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

  static build<K extends primitive, T>(a: Element<K, T>[]): void {
    for (let i = (a.length >> 1) - 1; i >= 0; i--) MinHeap.heapify(a, i, a.length);
  }

  build(): void {
    MinHeap.build(this.a);
  }

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

  /** Requires a non-empty heap. */
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

  /** Assumes both child subtrees are already heaps. */
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

  /** Requires a non-empty heap. */
  replace_root(e: Element<K, T>): void {
    this.a[0] = e;
    MaxHeap.heapify(this.a, 0, this.a.length);
  }
}

/** Unstable, in place, ascending by key. */
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
