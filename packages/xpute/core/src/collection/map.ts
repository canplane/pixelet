// @xpute/core/collection/map.ts

/**
 * A dense array of keys with an index into it: insert appends, delete moves
 * the last entry into the hole, and a walk over every entry is a walk over
 * contiguous memory.
 *
 * Laid out as lanes. The keys are one typed array in position order, and the
 * index from key to position is an open-addressing table of positions —
 * linear probing, deletion by backward shift, so no tombstones pile up. There
 * is no object per entry, and both halves follow the data into linear memory.
 *
 * Values are not stored. A position is the value: `insert` returns it,
 * `find` looks it up, and `remove_at` says which entry moved into a freed
 * position — a caller keeps what it stores per entry in its own lanes at the
 * same position and moves them along.
 *
 * Keys are integers of one width, chosen by KeyTraits: I64_KEYS for 64-bit
 * keys (a packed IVec, say), U32_KEYS for 32-bit ones. The two cannot share
 * one lane type — typed arrays split into a number family and a BigInt family
 * (abi/array.ts) — so the traits carry the lane, the hash and the canonical
 * form, the same way sparse_tree takes its key operations as PVecTraits.
 */

import type { i32, i64, numeric, u32, u64 } from "../abi/word.ts";
import type { AnyTypedArrayCtor, I64Array, U32Array } from "../abi/array.ts";
import { mix64 } from "../math/rng.ts";

type KeyLane = I64Array | U32Array;

export interface KeyTraits<K extends numeric> {
  readonly ctor: BigInt64ArrayConstructor | Uint32ArrayConstructor;
  /** The form a key is stored and compared in. */
  norm(key: K): K;
  /** A 32-bit mix whose high bits are well spread; the table takes a slot
   * from the top bits. */
  hash(key: K): u32;
}

/** 64-bit keys, compared as signed: a key past 2^63 — an unsigned packed
 * word — folds with asIntN on the way in, so the same number always finds
 * the same entry. `key_at` returns the folded form. */
export const I64_KEYS: KeyTraits<i64> = {
  ctor: BigInt64Array,
  norm: (k) => BigInt.asIntN(64, k),
  // Fibonacci hashing: the high half of k · 2^64/φ.
  hash: (k) => Number(BigInt.asUintN(64, k * 0x9e3779b97f4a7c15n) >> 32n),
};

export const U32_KEYS: KeyTraits<u32> = {
  ctor: Uint32Array,
  norm: (k) => k >>> 0,
  hash: (k) => Math.imul(k, 0x9e3779b1) >>> 0,
};

export class IndexedMap<K extends numeric> {
  private keys: KeyLane;
  /** Per slot, position + 1; 0 is an empty slot. */
  private table: U32Array;
  /** 32 − log2(table size): a hash's top bits pick the home slot. */
  private shift: u32;
  private mask: u32;
  private _size: u32 = 0;

  constructor(private readonly traits: KeyTraits<K>, cap: u32 = 16) {
    let size = 2;
    while (size < cap * 2) size <<= 1;
    this.table = new Uint32Array(size);
    this.keys = this.take_keys(size >>> 1);
    this.mask = size - 1;
    this.shift = 32 - Math.log2(size);
  }

  get size(): u32 {
    return this._size;
  }

  /** The key at a position, in its stored form. */
  key_at(pos: u32): K {
    return this.keys[pos] as K;
  }

  /** The key's position, or -1. */
  find(key: K): i32 {
    const k = this.traits.norm(key);
    let s = this.home(k);
    for (;;) {
      const e = this.table[s];
      if (e === 0) return -1;
      if (this.keys[e - 1] === k) return e - 1;
      s = (s + 1) & this.mask;
    }
  }

  /** The key's position, appending it at the end if it is new. */
  insert(key: K): u32 {
    const found = this.find(key);
    if (found >= 0) return found;
    if ((this._size + 1) * 2 > this.table.length) this.grow();
    const k = this.traits.norm(key);
    const pos = this._size++;
    this.keys[pos] = k as never;
    this.place(k, pos);
    return pos;
  }

  /**
   * Removes the entry at `pos` by moving the last entry into it. Returns the
   * position that entry came from — move your own lanes from there to `pos` —
   * or `pos` itself when the removed entry was the last.
   */
  remove_at(pos: u32): u32 {
    this.unplace(this.slot_of(this.keys[pos] as K));
    const last = --this._size;
    if (pos !== last) {
      const moved = this.keys[last] as K;
      this.table[this.slot_of(moved)] = pos + 1;
      this.keys[pos] = moved as never;
    }
    return last;
  }

  /** Forgets every entry; the capacity stays. */
  clear(): void {
    this.table.fill(0);
    this._size = 0;
  }

  /**
   * Every position once, in an order fixed by `seed`, in O(1) memory: a
   * start and a step coprime to the size walk the positions as a cycle. For
   * spreading work or routing without favoring insertion order. Mutating
   * during the walk is undefined.
   */
  *positions_seeded(seed: u64): IterableIterator<u32> {
    const n = this._size;
    if (n === 0) return;
    const s = mix64(seed);
    let pos = Number(s % BigInt(n));
    const step = coprime_step(n, mix64(s ^ 0x9e3779b97f4a7c15n));
    for (let t = 0; t < n; t++) {
      yield pos;
      pos = (pos + step) % n;
    }
  }

  private take_keys(n: u32): KeyLane {
    return new (this.traits.ctor as unknown as AnyTypedArrayCtor<KeyLane>)(n);
  }

  private home(k: K): u32 {
    return this.traits.hash(k) >>> this.shift;
  }

  private place(k: K, pos: u32): void {
    let s = this.home(k);
    while (this.table[s] !== 0) s = (s + 1) & this.mask;
    this.table[s] = pos + 1;
  }

  private slot_of(k: K): u32 {
    let s = this.home(k);
    while (this.keys[this.table[s] - 1] !== k) s = (s + 1) & this.mask;
    return s;
  }

  /** Backward-shift deletion: pull later entries of the probe run into the
   * hole unless their own home lies cyclically between the hole and them. */
  private unplace(hole: u32): void {
    let i = hole;
    let j = hole;
    this.table[i] = 0;
    for (;;) {
      j = (j + 1) & this.mask;
      const e = this.table[j];
      if (e === 0) return;
      const h = this.home(this.keys[e - 1] as K);
      const stays = i <= j ? h > i && h <= j : h > i || h <= j;
      if (!stays) {
        this.table[i] = e;
        this.table[j] = 0;
        i = j;
      }
    }
  }

  private grow(): void {
    const size = this.table.length * 2;
    const keys = this.take_keys(size >>> 1);
    keys.set(this.keys.subarray(0, this._size) as never);
    this.keys = keys;
    this.table = new Uint32Array(size);
    this.mask = size - 1;
    this.shift = 32 - Math.log2(size);
    for (let pos = 0; pos < this._size; pos++) this.place(keys[pos] as K, pos);
  }
}

function gcd(a: u32, b: u32): u32 {
  while (b !== 0) {
    const t = a % b;
    a = b;
    b = t;
  }
  return Math.abs(a);
}

function coprime_step(n: u32, seed: u64): u32 {
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
