// @xpute/core/ds/lanes.ts

/**
 * A table of typed lanes that move as one: every lane has the same length,
 * a row is one index across all of them, and growing, truncating and
 * swap-removing touch every lane together.
 *
 * What this replaces was N parallel `Vector`s per table — seventeen in the
 * chunk table, eleven in the scan's scratch — each pushed, truncated and
 * swap-copied by hand in the same order, with a capacity check per lane per
 * row. One length and one capacity here; a row is claimed with `push()` and
 * written through `mem`, so no per-row object is made and no lane can drift
 * from the others.
 *
 * `mem` holds the current views and is replaced whole when the table grows,
 * so a caller reads it fresh rather than caching a lane across a push.
 */

import type { u32 } from "../kit/word.ts";
import type { AnyTypedArray, AnyTypedArrayCtor } from "../kit/array.ts";
import { MarshalError } from "../sys/error.ts";
import { Errno } from "../sys/errno.ts";

export type LaneSpec = Record<string, AnyTypedArrayCtor<AnyTypedArray>>;
export type LaneViews<S extends LaneSpec> = { readonly [K in keyof S]: InstanceType<S[K]> };

export type LanesOptions = {
  init_cap?: u32;
  max_cap?: u32;
};

export class Lanes<S extends LaneSpec> {
  mem: LaneViews<S>;
  readonly max_cap: u32;
  private readonly keys: (keyof S)[];
  private _cap: u32;
  private _len: u32 = 0;

  constructor(readonly spec: S, opts: LanesOptions = {}) {
    const { init_cap = 1 << 10, max_cap = 1 << 24 } = opts;
    this.keys = Object.keys(spec) as (keyof S)[];
    this.max_cap = max_cap;
    this._cap = Math.min(init_cap, max_cap);
    this.mem = this.take(this._cap);
  }

  get len(): u32 {
    return this._len;
  }

  /** Claims the next row and returns its index; the lanes hold whatever
   * the previous occupant left, so the caller writes every lane it reads. */
  push(): u32 {
    if (this._len >= this._cap) this.grow(Math.max(this._cap * 2, 1));
    return this._len++;
  }

  truncate(new_len: u32): void {
    if (new_len < this._len) this._len = new_len;
  }

  clear(): void {
    this._len = 0;
  }

  /**
   * Frees row `i` by moving the last row into it. Returns the index the
   * moved row came from (the old last), which equals `i` when `i` was last
   * and nothing moved — so a caller keeping a handle per row knows which
   * handle to re-point.
   */
  swap_remove(i: u32): u32 {
    const last = --this._len;
    if (i !== last) {
      const mem = this.mem;
      for (const key of this.keys) {
        const lane = mem[key];
        lane[i] = lane[last] as never;
      }
    }
    return last;
  }

  private take(cap: u32): LaneViews<S> {
    const views: Partial<Record<keyof S, AnyTypedArray>> = {};
    for (const key of this.keys) views[key] = new this.spec[key](cap);
    return views as LaneViews<S>;
  }

  private grow(new_cap: u32): void {
    if (new_cap <= this._cap) return;
    if (new_cap > this.max_cap) throw new MarshalError(Errno.EOVERFLOW, `lanes: required size exceeds max_cap=${this.max_cap}`);
    const next = this.take(new_cap);
    for (const key of this.keys) {
      // The two views are always the same concrete family; TS cannot say so
      // across the number/bigint split.
      // deno-lint-ignore no-explicit-any
      (next[key] as any).set(this.mem[key].subarray(0, this._len));
    }
    this.mem = next;
    this._cap = new_cap;
  }
}
