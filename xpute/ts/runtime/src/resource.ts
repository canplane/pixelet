// @xpute/runtime/resource.ts

/**
 * A handle names an object on the side that serves it, which issued it
 * (xpute-core resource.rs). Handles come from a counter from 1, the root
 * being 0, and are never used again, so a frame late for a released object
 * reaches no other; issuing stops as a bug past the largest a command holds.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { HANDLE_MAX } from "./io/frame.ts";

export type Handle = u32;

/** The objects a side serves, by handle. */
export class Table<T> {
  private readonly _map = new Map<Handle, T>();
  private _next: Handle = 1;

  /** Puts `v` at the next handle, and answers it. */
  add(v: T): Handle {
    const h = this._next;
    if (h > HANDLE_MAX) throw new InvariantError(Errno.EMFILE);
    this._next = h + 1;
    this._map.set(h, v);
    return h;
  }

  get(h: Handle): T | undefined {
    return this._map.get(h);
  }

  remove(h: Handle): T | undefined {
    const v = this._map.get(h);
    this._map.delete(h);
    return v;
  }
}
