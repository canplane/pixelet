// @xpute/kit/collection/map.ts

/**
 * This is an index from keys to dense positions. The keys sit in an array with
 * no holes, and an open-addressing table, with linear probing and
 * backward-shift deletion, finds a key's position.
 *
 * The map stores no values: a caller keeps each entry's data in arrays of its
 * own at the entry's position, and moves that data when `remove_at` moves an
 * entry.
 */

import type { i32, i64, numeric, u32, u64 } from "../abi/word.ts";
import type { AnyTypedArrayCtor, I64Array, U32Array } from "../abi/array.ts";
import { mix64 } from "../math/rng.ts";

type KeyLane = I64Array | U32Array;

export interface KeyType<K extends numeric> {
  readonly ctor: BigInt64ArrayConstructor | Uint32ArrayConstructor;
  norm(key: K): K;
  /**
   * A 32-bit hash whose high bits are well spread, because the table takes a
   * key's home slot from the top bits.
   */
  hash(key: K): u32;
}

/**
 * 64-bit keys, compared as signed. A key at or above 2^63, such as an unsigned
 * packed word, is folded into the signed range on the way in, so the same 64
 * bits always find the same entry. `key_at` returns the folded form.
 */
export const I64_KEYS: KeyType<i64> = {
  ctor: BigInt64Array,
  norm: (k) => BigInt.asIntN(64, k),
  // Fibonacci hashing: the high half of k · 2^64/φ.
  hash: (k) => Number(BigInt.asUintN(64, k * 0x9e3779b97f4a7c15n) >> 32n),
};

export const U32_KEYS: KeyType<u32> = {
  ctor: Uint32Array,
  norm: (k) => k >>> 0,
  hash: (k) => Math.imul(k, 0x9e3779b1) >>> 0,
};

export class IndexedMap<K extends numeric> {
  private _keys: KeyLane;
  /** Each slot holds a position plus one; 0 marks an empty slot. */
  private _table: U32Array;
  /**
   * 32 − log2(table size), so that a hash shifted right by it leaves the top
   * bits, which pick a key's home slot.
   */
  private _shift: u32;
  private _mask: u32;
  private _size: u32 = 0;

  private readonly _traits: KeyType<K>;

  constructor(traits: KeyType<K>, cap: u32 = 16) {
    this._traits = traits;
    let sz = 2;
    while (sz < cap * 2) sz <<= 1;
    this._table = new Uint32Array(sz);
    this._keys = this._take_keys(sz >>> 1);
    this._mask = sz - 1;
    this._shift = 32 - Math.log2(sz);
  }

  get size(): u32 {
    return this._size;
  }

  key_at(pos: u32): K {
    return this._keys[pos] as K;
  }

  find(key: K): i32 {
    const k = this._traits.norm(key);
    let s = this._home(k);
    for (;;) {
      const e = this._table[s];
      if (e === 0) return -1;
      if (this._keys[e - 1] === k) return e - 1;
      s = (s + 1) & this._mask;
    }
  }

  insert(key: K): u32 {
    const found = this.find(key);
    if (found >= 0) return found;
    if ((this._size + 1) * 2 > this._table.length) this._grow();
    const k = this._traits.norm(key);
    const pos = this._size++;
    this._keys[pos] = k as never;
    this._place(k, pos);
    return pos;
  }

  /**
   * Removes the entry at `pos` by moving the last entry into its place.
   * Returns the position the moved entry came from, so that the caller moves
   * its own data from there to `pos`; when the removed entry was the last,
   * that is `pos` itself.
   */
  remove_at(pos: u32): u32 {
    this._unplace(this._slot_of(this._keys[pos] as K));
    const last = --this._size;
    if (pos !== last) {
      const moved = this._keys[last] as K;
      this._table[this._slot_of(moved)] = pos + 1;
      this._keys[pos] = moved as never;
    }
    return last;
  }

  /** Forgets every entry; the capacity stays. */
  clear(): void {
    this._table.fill(0);
    this._size = 0;
  }

  /**
   * Yields every position once, in an order fixed by `seed`. The order is a
   * stride from a start, not a uniform shuffle. The map must not change
   * during the walk.
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

  private _take_keys(n: u32): KeyLane {
    return new (this._traits.ctor as unknown as AnyTypedArrayCtor<KeyLane>)(n);
  }

  private _home(k: K): u32 {
    return this._traits.hash(k) >>> this._shift;
  }

  private _place(k: K, pos: u32): void {
    let s = this._home(k);
    while (this._table[s] !== 0) s = (s + 1) & this._mask;
    this._table[s] = pos + 1;
  }

  private _slot_of(k: K): u32 {
    let s = this._home(k);
    while (this._keys[this._table[s] - 1] !== k) s = (s + 1) & this._mask;
    return s;
  }

  /**
   * Backward-shift deletion. Each later entry of the probe run moves back
   * into the hole, unless its own home slot lies cyclically between the hole
   * and the entry.
   */
  private _unplace(hole: u32): void {
    let i = hole;
    let j = hole;
    this._table[i] = 0;
    for (;;) {
      j = (j + 1) & this._mask;
      const e = this._table[j];
      if (e === 0) return;
      const h = this._home(this._keys[e - 1] as K);
      const stays = i <= j ? h > i && h <= j : h > i || h <= j;
      if (!stays) {
        this._table[i] = e;
        this._table[j] = 0;
        i = j;
      }
    }
  }

  private _grow(): void {
    const sz = this._table.length * 2;
    const keys = this._take_keys(sz >>> 1);
    keys.set(this._keys.subarray(0, this._size) as never);
    this._keys = keys;
    this._table = new Uint32Array(sz);
    this._mask = sz - 1;
    this._shift = 32 - Math.log2(sz);
    for (let pos = 0; pos < this._size; pos++) this._place(keys[pos] as K, pos);
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
