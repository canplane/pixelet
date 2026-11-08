// @xpute/kit/collection/heap.ts
// Binary min-heaps and max-heaps over an array the caller owns. They check
// nothing; a key that compares with nothing, as a NaN does, breaks the heap's
// order.

import type { primitive, u32 } from "../abi/word.ts";

export interface Element<K extends primitive, T> {
  key: K;
  val: T;
}

export class MinHeap<K extends primitive, T> {
  readonly a: Element<K, T>[];

  constructor(a: Element<K, T>[]) {
    this.a = a;
  }

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
   * Sifts the element at `root` down within the first `sz` elements. Both
   * subtrees of `root` must already be heaps.
   */
  static heapify<K extends primitive, T>(
    a: Element<K, T>[],
    root: u32,
    sz: u32,
  ): void {
    let i = root;
    const e = a[i];

    for (let child = (i << 1) + 1; child < sz; child = (i << 1) + 1) {
      const r = child + 1;
      if (r < sz && a[r].key < a[child].key) child = r;

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
    for (let i = (a.length >> 1) - 1; i >= 0; i--) {
      MinHeap.heapify(a, i, a.length);
    }
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

  /**
   * Replaces the top element and restores the order. The heap must not be
   * empty.
   */
  replace_root(e: Element<K, T>): void {
    this.a[0] = e;
    MinHeap.heapify(this.a, 0, this.a.length);
  }
}

export class MaxHeap<K extends primitive, T> {
  readonly a: Element<K, T>[];

  constructor(a: Element<K, T>[]) {
    this.a = a;
  }

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
   * Sifts the element at `root` down within the first `sz` elements. Both
   * subtrees of `root` must already be heaps.
   */
  static heapify<K extends primitive, T>(
    a: Element<K, T>[],
    root: u32,
    sz: u32,
  ): void {
    let i = root;
    const e = a[i];

    for (let child = (i << 1) + 1; child < sz; child = (i << 1) + 1) {
      const r = child + 1;
      if (r < sz && a[r].key > a[child].key) child = r;

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
    for (let i = (a.length >> 1) - 1; i >= 0; i--) {
      MaxHeap.heapify(a, i, a.length);
    }
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

  /**
   * Replaces the top element and restores the order. The heap must not be
   * empty.
   */
  replace_root(e: Element<K, T>): void {
    this.a[0] = e;
    MaxHeap.heapify(this.a, 0, this.a.length);
  }
}

/** Sorts by key, ascending and in place. The sort is not stable. */
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
