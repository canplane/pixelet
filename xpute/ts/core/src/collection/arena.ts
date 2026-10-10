// @xpute/core/collection/arena.ts

// Bump allocators with mark/rewind: Arena reuses preconstructed objects, which
// alloc() hands out uninitialized; Vector is typed-array scratch.

import type { u32 } from "../abi/word.ts";
import type { AnyElementOf, AnyTypedArray, AnyTypedArrayCtor } from "../abi/array.ts";
import { InvariantError, MarshalError } from "@xpute/core/status/error.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";

export interface ArenaOptions {
  init_cap?: u32;
  max_cap?: u32;
}

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

  alloc(): T {
    if (this._len >= this._cap) this._grow(Math.max(this._cap * 2, 1));

    return this.mem[this._len++];
  }

  reserve(cnt: u32): void {
    const req = this._len + cnt;
    if (req > this._cap) this._grow(Math.max(this._cap * 2, req));
  }

  /** A `new_len` past the current length is a broken invariant, not a clamp. */
  truncate(new_len: u32): void {
    if (new_len > this._len) {
      throw new InvariantError(Errno.EINVAL);
    }
    this._len = new_len;
  }

  private _grow(new_cap: u32): void {
    if (new_cap <= this._cap) return;
    if (new_cap > this.max_cap) throw new MarshalError(Errno.EOVERFLOW);

    this.mem.length = new_cap;
    for (let i = this._cap; i < new_cap; i++) this.mem[i] = this._factory();
    this._cap = new_cap;
  }

  get scope(): ArenaGuard<T> {
    return new ArenaGuard(this);
  }
}

class ArenaGuard<T extends object> implements Disposable {
  private readonly _mark: u32;

  constructor(private readonly arena: Arena<T>) {
    this._mark = arena.len;
  }

  [Symbol.dispose]() {
    this.arena.truncate(this._mark);
  }
}

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

  set(idx: u32, v: AnyElementOf<T>): void {
    if (idx >= this._cap) this._grow(Math.max(this._cap * 2, idx + 1));

    this.mem[idx] = v;
    if (idx >= this._len) this._len = idx + 1;
  }

  push(v: AnyElementOf<T>): u32 {
    if (this._len >= this._cap) this._grow(Math.max(this._cap * 2, 1));

    const i = this._len++;
    this.mem[i] = v;
    return i;
  }

  reserve(cnt: u32): void {
    const req = this._len + cnt;
    if (req > this._cap) this._grow(Math.max(this._cap * 2, req));
  }

  truncate(new_len: u32): void {
    if (new_len > this._len) {
      console.warn(`[vector] truncate ignored: new_len=${new_len} > len=${this._len}`);
      return;
    }
    this._len = new_len;
  }

  private _grow(new_cap: u32): void {
    if (new_cap <= this._cap) return;
    if (new_cap > this.max_cap) throw new MarshalError(Errno.EOVERFLOW);

    const new_mem: T = new this.ctor(new_cap);
    // `set` cannot be typed across the number and bigint array families;
    // both arrays are always the same one.
    // deno-lint-ignore no-explicit-any
    new_mem.set(this.mem as any);

    this.mem = new_mem;
    this._cap = new_cap;
  }

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
