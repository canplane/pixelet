// @xpute/core/ds/handle.ts

/**
 * Handles to what another side keeps: the number is the table's, the thing it
 * names is the holder's — the way an OS hands out a descriptor and keeps the
 * file. A handle packs a slot and that slot's generation, so a handle kept
 * past its release does not name whatever took the slot after it.
 *
 * The table is fixed at its capacity and never grows: past it `acquire`
 * refuses. Slot 0 is never handed out, so 0 is never a handle.
 */

import type { u32 } from "../kit/word.ts";

/** Bits of a handle that are its slot; the rest are the slot's generation. */
export const SLOT_BITS: u32 = 16;
const SLOT_MASK = (1 << SLOT_BITS) - 1;
const NONE = 0xffffffff;

/** The slot a handle names. */
export function handle_slot(handle: u32): u32 {
  return handle & SLOT_MASK;
}

export class HandleTable {
  private readonly generation: Uint16Array;
  private readonly alive: Uint8Array;
  /** The free list's links, by slot. */
  private readonly next: Uint32Array;
  private free = NONE;
  /** Slots never handed out start here. */
  private fresh = 1;
  private live_count = 0;

  constructor(private readonly slots: u32) {
    if (slots < 2 || slots > 1 << SLOT_BITS) throw new Error(`a handle table holds 2 to 2^${SLOT_BITS} slots`);
    this.generation = new Uint16Array(slots);
    this.alive = new Uint8Array(slots);
    this.next = new Uint32Array(slots).fill(NONE);
  }

  /** A handle to a slot of its own, or null when every slot is taken. */
  acquire(): u32 | null {
    let slot: u32;
    if (this.free !== NONE) {
      slot = this.free;
      this.free = this.next[slot];
    } else if (this.fresh < this.slots) {
      slot = this.fresh++;
    } else {
      return null;
    }
    this.generation[slot] = Math.max(1, (this.generation[slot] + 1) & 0xffff);
    this.alive[slot] = 1;
    this.live_count++;
    return (slot | (this.generation[slot] << SLOT_BITS)) >>> 0;
  }

  /** Gives the handle's slot back. False for a handle that is not live. */
  release(handle: u32): boolean {
    if (!this.live(handle)) return false;
    const slot = handle_slot(handle);
    this.alive[slot] = 0;
    this.next[slot] = this.free;
    this.free = slot;
    this.live_count--;
    return true;
  }

  /** Whether the handle names its slot now. */
  live(handle: u32): boolean {
    const slot = handle_slot(handle);
    return slot !== 0 && slot < this.slots && this.alive[slot] === 1 && this.generation[slot] === handle >>> SLOT_BITS;
  }

  /** Handles live now. */
  count(): u32 {
    return this.live_count;
  }
}
