// @xpute/link/resource.ts

/**
 * A handle names an object on the side that implements it, which issued it
 * (xpute-link resource.rs). As the component model's table does, a handle's
 * slot is used again once its object is dropped, so that a side issues
 * handles for as long as it runs, the slots dropped first used first, so
 * that each comes back as late as it can; the slot is the handle's low
 * `SLOT_BITS`, from 1, the root being 0. Above it a generation, which a slot's drop
 * advances, keeps a frame late for a dropped object from reaching the one in
 * its slot now, until the slot has been used `GENERATION_MASK` + 1 times. More live
 * objects than the slots hold is a bug.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EBADF, ERRNO_EMFILE } from "@xpute/kit/status/errno.spec.ts";
import { bug } from "@xpute/kit/status/bug.ts";
import { NAMED, SLOT_BITS } from "./frame.spec.ts";

export type Handle = u32;

const SLOT_MASK = (1 << SLOT_BITS) - 1;
/** The largest generation an issued handle holds above its slot, below `NAMED`. */
const GENERATION_MASK = (NAMED - 1) >>> SLOT_BITS;

/** What an issued slot's `next` holds, which no free slot's does. */
const ISSUED = 0xffff_ffff;

/** A slot: its generation, its object while it holds one, and the next free
 * slot while it is free, 0 for none; `ISSUED` while it is issued. */
type Slot<T> = { generation: u32; val: T | undefined; next: u32 };

/** The objects a side implements, by handle: slot `k` at `k - 1`, and the queue
 * of the slots dropped, through their `next`, from `_head` to `_tail`. */
export class Table<T> {
  private readonly _slots: Slot<T>[] = [];
  private _head: u32 = 0;
  private _tail: u32 = 0;

  /** Issues a handle, for `put` to fill; `remove` gives it back unfilled. */
  issue(): Handle {
    let k = this._head;
    if (k !== 0) {
      const s = this._slots[k - 1]!;
      this._head = s.next;
      s.next = ISSUED;
      if (this._head === 0) this._tail = 0;
    } else {
      this._slots.push({ generation: 0, val: undefined, next: ISSUED });
      k = this._slots.length;
      if (k > SLOT_MASK) bug(ERRNO_EMFILE);
    }
    return (k | (this._slots[k - 1]!.generation << SLOT_BITS)) >>> 0;
  }

  /** The slot `h` names, where `h` is of its generation. */
  private _slot(h: Handle): Slot<T> | undefined {
    const s = this._slots[(h & SLOT_MASK) - 1];
    return s !== undefined && s.generation === h >>> SLOT_BITS ? s : undefined;
  }

  /** Puts `v` under `h`: a handle this table issued, or one the other end
   * issued, as a UI's end holds what the runtime opens on it under the
   * guest's end's handles. The slot `h` names is then issued at `h`'s
   * generation, taken out of the queue if it was free, it and the slots
   * before it made free where there were none; one issued at another
   * generation is a bug. */
  put(h: Handle, v: T): void {
    const k = h & SLOT_MASK;
    if (k === 0) bug(ERRNO_EBADF);
    while (this._slots.length < k) {
      this._slots.push({ generation: 0, val: undefined, next: ISSUED });
      this.remove(this._slots.length);
    }
    const s = this._slots[k - 1]!;
    if (s.next !== ISSUED) {
      let prev = 0;
      for (let c = this._head; c !== k; c = this._slots[c - 1]!.next) prev = c;
      if (prev === 0) this._head = s.next;
      else this._slots[prev - 1]!.next = s.next;
      if (this._tail === k) this._tail = prev;
      s.next = ISSUED;
      s.generation = h >>> SLOT_BITS;
    }
    if (s.generation !== h >>> SLOT_BITS) bug(ERRNO_EBADF);
    s.val = v;
  }

  /** Puts `v` at a handle issued for it, and answers the handle. */
  add(v: T): Handle {
    const h = this.issue();
    this.put(h, v);
    return h;
  }

  get(h: Handle): T | undefined {
    return this._slot(h)?.val;
  }

  /** Drops `h`: its object, if it holds one, and its slot, last in the
   * queue, for a later handle of the next generation. A slot that is free
   * already is no handle's. */
  remove(h: Handle): T | undefined {
    const s = this._slot(h);
    if (s === undefined || s.next !== ISSUED) return undefined;
    const v = s.val;
    s.val = undefined;
    s.generation = (s.generation + 1) & GENERATION_MASK;
    s.next = 0;
    const k = h & SLOT_MASK;
    if (this._tail === 0) this._head = k;
    else this._slots[this._tail - 1]!.next = k;
    this._tail = k;
    return v;
  }
}
