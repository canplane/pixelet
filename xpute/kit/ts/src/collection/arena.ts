// @xpute/kit/collection/arena.ts

/**
 * Arena / Vector scratch allocators (bump + mark/rewind)
 * - Arena: reusable object slots
 * - Vector: reusable typed-array scratch
 */

import type { u32 } from "../abi/word.ts";
import type {
  AnyElementOf,
  AnyTypedArray,
  AnyTypedArrayCtor,
} from "../abi/array.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { bug } from "@xpute/kit/status/bug.ts";
import { ERRNO_EINVAL, ERRNO_EOVERFLOW } from "@xpute/kit/status/errno.spec.ts";

export interface ArenaConfig {
  init_cap: u32;
  max_cap: u32;
}

// ============ Arena ============

/**
 * Arena allocator with bump allocation and mark/rewind reset.
 *
 * Intended for transient objects to reduce GC churn.
 * - alloc: bump cursor (amortized O(1); growth constructs the new slots)
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
  private _len: u32 = 0;

  private _factory: () => T;

  constructor(factory: () => T, cfg: ArenaConfig) {
    const { init_cap: _init_cap, max_cap } = cfg;
    this.max_cap = max_cap;

    this._factory = factory;
    this._grow(Math.min(_init_cap, max_cap));
  }

  get cap(): u32 {
    return this.mem.length;
  }

  get len(): u32 {
    return this._len;
  }

  get(idx: u32): T {
    return this.mem[idx];
  }

  /** Allocate one object from the arena. */
  alloc(): T {
    if (this._len >= this.cap) this._grow(this._len + 1);

    return this.mem[this._len++];
  }

  /** Grow now so that the next cnt allocations do not; throws past `max_cap`. */
  reserve(cnt: u32): void {
    this._grow(this._len + cnt);
  }

  /**
   * Rewind the bump cursor to a previous position.
   * A `new_len` above the current length is a bug, not a way to grow.
   */
  truncate(new_len: u32): void {
    if (new_len > this._len) {
      bug(ERRNO_EINVAL);
    }
    this._len = new_len;
  }

  // The capacity doubles but stops at `max_cap`, so a request is refused
  // only when it does not fit under `max_cap`.
  private _grow(req: u32): void {
    const cap = this.cap;
    if (req <= cap) return;
    if (req > this.max_cap) throw new XputeError(ERRNO_EOVERFLOW);
    const new_cap = Math.min(Math.max(cap * 2, req), this.max_cap);

    this.mem.length = new_cap; // V8 Array pre-allocation hint
    for (let i = cap; i < new_cap; i++) this.mem[i] = this._factory();
  }

  /** Scope guard that rewinds to the current mark on dispose. */
  get scope(): ArenaGuard<T> {
    return new ArenaGuard(this);
  }
}

/** Scope guard that rewinds an arena to its saved mark on dispose. */
class ArenaGuard<T extends object> implements Disposable {
  private readonly _mark: u32;

  private readonly arena: Arena<T>;

  constructor(arena: Arena<T>) {
    this.arena = arena;
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
 * - set : random write, at most one past the current len
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
  private _len: u32 = 0;

  readonly ctor: AnyTypedArrayCtor<T>;

  constructor(ctor: AnyTypedArrayCtor<T>, cfg: ArenaConfig) {
    this.ctor = ctor;
    const { init_cap: _init_cap, max_cap } = cfg;
    this.max_cap = max_cap;

    this.mem = new ctor(Math.min(_init_cap, max_cap));
  }

  get cap(): u32 {
    return this.mem.length;
  }

  get len(): u32 {
    return this._len;
  }

  get(idx: u32): AnyElementOf<T> {
    return this.mem[idx] as AnyElementOf<T>;
  }

  /**
   * Sets the element at `idx`, at most one past the length, where it is
   * pushed. A gap is refused: the elements in it would still hold what a
   * truncate left behind.
   */
  set(idx: u32, v: AnyElementOf<T>): void {
    if (idx > this._len) bug(ERRNO_EINVAL);
    if (idx === this._len) this.push(v);
    else this.mem[idx] = v;
  }

  /** Push one primitive value into scratch. Returns the written index. */
  push(v: AnyElementOf<T>): u32 {
    if (this._len >= this.cap) this._grow(this._len + 1);

    const i = this._len++;
    this.mem[i] = v;
    return i;
  }

  /** Grow now so that the next cnt pushes do not; throws past `max_cap`. */
  reserve(cnt: u32): void {
    this._grow(this._len + cnt);
  }

  /**
   * Rewind the bump cursor to a previous position.
   * A `new_len` above the current length is a bug, not a way to grow.
   */
  truncate(new_len: u32): void {
    if (new_len > this._len) bug(ERRNO_EINVAL);
    this._len = new_len;
  }

  /**
   * Grow the backing store and copy existing contents.
   *
   * Note:
   * - this is the only path that reallocates the backing store
   * - if this triggers frequently, increase init_cap
   * - the capacity doubles but stops at `max_cap`, so a request is refused
   *   only when it does not fit under `max_cap`
   */
  private _grow(req: u32): void {
    if (req <= this.cap) return;
    if (req > this.max_cap) throw new XputeError(ERRNO_EOVERFLOW);
    const new_cap = Math.min(Math.max(this.cap * 2, req), this.max_cap);

    const new_mem: T = new this.ctor(new_cap);
    // TypeScript cannot type `set` across the number and bigint array
    // families, but the two arrays here are always of the same family.
    new_mem.set(this.mem as any);

    this.mem = new_mem;
  }

  /** Scope guard that rewinds to the current mark on dispose. */
  get scope(): VectorGuard<T> {
    return new VectorGuard(this);
  }
}

class VectorGuard<T extends AnyTypedArray> implements Disposable {
  private readonly _mark: u32;

  private readonly vec: Vector<T>;

  constructor(vec: Vector<T>) {
    this.vec = vec;
    this._mark = vec.len;
  }

  [Symbol.dispose]() {
    this.vec.truncate(this._mark);
  }
}
