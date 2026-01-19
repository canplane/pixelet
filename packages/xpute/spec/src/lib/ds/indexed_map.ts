// @xpute/spec/lib/ds/indexed_map.ts
// Array + Map index. O(1) insert/delete, fast iteration, seeded permutation iterator.

export type Key = string | number | bigint;

export class IndexedMap<K extends Key, V> implements Iterable<[K, V]> {
  private readonly entries: Array<[K, V]> = [];
  private readonly index: Map<K, number> = new Map();

  get size(): number {
    return this.entries.length;
  }

  has(k: K): boolean {
    return this.index.has(k);
  }

  get(k: K): V | undefined {
    const i = this.index.get(k);
    return i === undefined ? undefined : this.entries[i][1];
  }

  // O(1) insert; returns false if exists
  insert(k: K, v: V): boolean {
    if (this.index.has(k)) return false;
    this.index.set(k, this.entries.length);
    this.entries.push([k, v]);
    return true;
  }

  // O(1) upsert
  upsert(k: K, v: V): void {
    const i = this.index.get(k);
    if (i === undefined) {
      this.index.set(k, this.entries.length);
      this.entries.push([k, v]);
    } else {
      this.entries[i][1] = v;
    }
  }

  // O(1) delete via swap-with-last
  delete(k: K): boolean {
    const i = this.index.get(k);
    if (i === undefined) return false;

    const last = this.entries.length - 1;
    if (i !== last) {
      const lastEntry = this.entries[last];
      this.entries[i] = lastEntry;
      this.index.set(lastEntry[0], i);
    }
    this.entries.pop();
    this.index.delete(k);
    return true;
  }

  clear(): void {
    this.entries.length = 0;
    this.index.clear();
  }

  // Default iteration: array order (fast)
  *[Symbol.iterator](): IterableIterator<[K, V]> {
    for (let i = 0; i < this.entries.length; i++) yield this.entries[i];
  }

  *keys(): IterableIterator<K> {
    for (let i = 0; i < this.entries.length; i++) yield this.entries[i][0];
  }

  *values(): IterableIterator<V> {
    for (let i = 0; i < this.entries.length; i++) yield this.entries[i][1];
  }

  // Seeded full permutation iterator in O(1) memory.
  // NOTE: snapshot semantics: if you mutate during iteration, behavior is undefined.
  *iterSeeded(seed: bigint): IterableIterator<[K, V]> {
    const n = this.entries.length;
    if (n === 0) return;

    const s = splitmix64(seed);
    const start = Number(s % BigInt(n));
    const step = makeCoprimeStep(n, splitmix64(s ^ 0x9e3779b97f4a7c15n));

    let idx = start;
    for (let t = 0; t < n; t++) {
      yield this.entries[idx];
      idx += step;
      idx %= n;
    }
  }

  *keysSeeded(seed: bigint): IterableIterator<K> {
    for (const [k] of this.iterSeeded(seed)) yield k;
  }

  *valuesSeeded(seed: bigint): IterableIterator<V> {
    for (const [, v] of this.iterSeeded(seed)) yield v;
  }
}

/** -------- helpers -------- */

function gcd(a: number, b: number): number {
  while (b !== 0) {
    const t = a % b;
    a = b;
    b = t;
  }
  return Math.abs(a);
}

function makeCoprimeStep(n: number, seed: bigint): number {
  if (n === 1) return 1;
  let step = Number(seed % BigInt(n));
  if (step === 0) step = 1;
  step = (step | 1) % n;
  if (step === 0) step = 1;
  while (gcd(step, n) !== 1) {
    step = (step + 2) % n;
    if (step === 0) step = 1;
  }
  return step;
}

// SplitMix64 PRNG (BigInt). Good enough for routing randomization.
function splitmix64(x: bigint): bigint {
  x = (x + 0x9e3779b97f4a7c15n) & 0xffffffffffffffffn;
  let z = x;
  z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & 0xffffffffffffffffn;
  z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & 0xffffffffffffffffn;
  return z ^ (z >> 31n);
}
