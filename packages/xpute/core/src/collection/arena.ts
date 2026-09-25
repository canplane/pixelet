// @xpute/core/collection/arena.ts

/**
 * Arena / Vector scratch allocators (bump + mark/rewind)
 * - Arena: reusable object slots
 * - Vector: reusable typed-array scratch
 */

import type { u32 } from "../abi/word.ts";
import type { AnyElementOf, AnyTypedArray, AnyTypedArrayCtor } from "../abi/array.ts";
import { InvariantError, MarshalError } from "@xpute/core/status/error.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";

export interface ArenaOptions {
  init_cap?: u32; // soft initial size hint
  max_cap?: u32; // hard size limit
}

// ============ Arena ============

/**
 * Arena allocator with bump allocation and mark/rewind reset.
 *
 * Intended for transient objects to reduce GC churn.
 * - alloc: bump cursor (O(1))
 * - truncate: rewind by mark/scope (bulk reset)
 *
 * Notes:
 * - objects are preconstructed and reused
 * - alloc() does not initialize object fields
 * - caller owns object reinitialization before use
 */
export class Arena<T extends object> {
  readonly mem: T[] = [];

  readonly max_cap: u32;
  private _cap: u32 = 0;
  private _len: u32 = 0;

  private _factory: () => T;

  constructor(factory: () => T, opts: ArenaOptions = {}) {
    const { init_cap: _init_cap = 1 << 10, max_cap = 1 << 24 } = opts;
    this.max_cap = max_cap;
    this._cap = 0;

    this._factory = factory;
    this._grow(Math.min(_init_cap, max_cap));
  }

  get cap(): u32 {
    return this._cap;
  }

  get len(): u32 {
    return this._len;
  }

  get(idx: u32): T {
    return this.mem[idx];
  }

  /** Allocate one object from the arena. */
  alloc(): T {
    if (this._len >= this._cap) this._grow(Math.max(this._cap * 2, 1));

    return this.mem[this._len++];
  }

  /** Ensure space for cnt additional allocations without growing. */
  reserve(cnt: u32): void {
    const req = this._len + cnt;
    if (req > this._cap) this._grow(Math.max(this._cap * 2, req));
  }

  /**
   * Rewind the bump cursor to a previous position.
   *
   * Kernel rule:
   * - new_len validity is caller-owned
   * - a new_len past the current len is a broken invariant, not a clamp
   */
  truncate(new_len: u32): void {
    if (new_len > this._len) {
      throw new InvariantError(Errno.EINVAL);
    }
    this._len = new_len;
  }

  private _grow(new_cap: u32): void {
    if (new_cap <= this._cap) return;
    if (new_cap > this.max_cap) throw new MarshalError(Errno.EOVERFLOW);

    this.mem.length = new_cap; // V8 Array pre-allocation hint
    for (let i = this._cap; i < new_cap; i++) this.mem[i] = this._factory();
    this._cap = new_cap;
  }

  /** Scope guard that rewinds to the current mark on dispose. */
  get scope(): ArenaGuard<T> {
    return new ArenaGuard(this);
  }
}

/** Scope guard that rewinds an arena to its saved mark on dispose. */
class ArenaGuard<T extends object> implements Disposable {
  private readonly _mark: u32;

  constructor(private readonly arena: Arena<T>) {
    this._mark = arena.len;
  }

  [Symbol.dispose]() {
    this.arena.truncate(this._mark);
  }
}

// ============ Vector ============

/**
 * Contiguous typed scratch store with bump write and mark/rewind control.
 *
 * Intended for transient primitive data to reduce GC churn.
 * - push: append one element (amortized O(1))
 * - set : random write with auto-grow and len extension
 * - truncate: rewind by mark/scope (bulk reset)
 *
 * Notes:
 * - this is a scratch allocator, not a general container
 * - reserve(cnt) means additional capacity from the current len
 * - logical length is tracked separately from typed-array capacity
 */
export class Vector<T extends AnyTypedArray> {
  mem: T;

  readonly max_cap: u32;
  private _cap: u32;
  private _len: u32 = 0;

  constructor(readonly ctor: AnyTypedArrayCtor<T>, opts: ArenaOptions = {}) {
    const { init_cap: _init_cap = 1 << 10, max_cap = 1 << 24 } = opts;
    this.max_cap = max_cap;
    this._cap = Math.min(_init_cap, max_cap);

    this.mem = new ctor(this._cap);
  }

  get cap(): u32 {
    return this._cap;
  }

  get len(): u32 {
    return this._len;
  }

  get(idx: u32): AnyElementOf<T> {
    return this.mem[idx] as AnyElementOf<T>;
  }

  /**
   * Random write at idx.
   *
   * Semantics:
   * - grows capacity if needed
   * - extends len to idx + 1 if idx is beyond the current logical end
   */
  set(idx: u32, v: AnyElementOf<T>): void {
    if (idx >= this._cap) this._grow(Math.max(this._cap * 2, idx + 1));

    this.mem[idx] = v;
    if (idx >= this._len) this._len = idx + 1;
  }

  /** Push one primitive value into scratch. Returns the written index. */
  push(v: AnyElementOf<T>): u32 {
    if (this._len >= this._cap) this._grow(Math.max(this._cap * 2, 1));

    const i = this._len++;
    this.mem[i] = v;
    return i;
  }

  /** Ensure space for cnt additional pushes without growing. */
  reserve(cnt: u32): void {
    const req = this._len + cnt;
    if (req > this._cap) this._grow(Math.max(this._cap * 2, req));
  }

  /**
   * Rewind the bump cursor to a previous position.
   *
   * Kernel rule:
   * - new_len validity is caller-owned
   * - if new_len exceeds the current len, the request is ignored with a warning
   */
  truncate(new_len: u32): void {
    if (new_len > this._len) {
      console.warn(`[vector] truncate ignored: new_len=${new_len} > len=${this._len}`);
      return;
    }
    this._len = new_len;
  }

  /**
   * Grow the backing store and copy existing contents.
   *
   * Note:
   * - this is the only allocation path after construction
   * - if this triggers frequently, increase init_cap
   */
  private _grow(new_cap: u32): void {
    if (new_cap <= this._cap) return;
    if (new_cap > this.max_cap) throw new MarshalError(Errno.EOVERFLOW);

    const new_mem: T = new this.ctor(new_cap);
    // AnyTypedArray spans both the number and bigint families — TS can't
    // express ".set() accepts this same generic T" across that split
    // (it would need ArrayLike<number> & ArrayLike<bigint>, which nothing
    // satisfies); this.mem and new_mem are always the same concrete family.
    // deno-lint-ignore no-explicit-any
    new_mem.set(this.mem as any);

    this.mem = new_mem;
    this._cap = new_cap;
  }

  /** Scope guard that rewinds to the current mark on dispose. */
  get scope(): VectorGuard<T> {
    return new VectorGuard(this);
  }
}

class VectorGuard<T extends AnyTypedArray> implements Disposable {
  private readonly _mark: u32;

  constructor(private readonly vector: Vector<T>) {
    this._mark = vector.len;
  }

  [Symbol.dispose]() {
    this.vector.truncate(this._mark);
  }
}
