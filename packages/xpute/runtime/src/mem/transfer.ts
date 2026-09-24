// @xpute/runtime/mem/transfer.ts

/**
 * The little memory: transferable `ArrayBuffer`s for work that leaves the
 * thread, the central scheduler's own. A `WebAssembly.Memory` cannot be
 * transferred, so a job bound for a worker carries its bytes in a buffer of
 * its own and brings the buffer back
 * (issue/20260911-single-thread-core-and-when-to-spawn-a-worker.md, "Big and
 * little, decided"): fixed-size, one job's worth; made when first needed,
 * reused when it comes back, released once idle; capped, and the cap is the
 * backpressure.
 *
 * Nothing borrows one yet: no worker exists. When one does, a job is a guest's
 * I/O demand (xpute-runtime io.rs), so a buffer is owned by that demand's key, and the
 * buffers not away are the worker credits the central scheduler grants.
 */

import type { f64, u32, u64 } from "@xpute/core/abi/word.ts";
import { Vector } from "@xpute/core/collection/arena.ts";

const NONE = 0;
const HOME = 1;
const AWAY = 2;

const LANE = { init_cap: 8 } as const;

export class TransferPool {
  private readonly state = new Vector(Uint8Array, LANE);
  /** The demand holding the buffer (xpute-runtime io.rs's key), while AWAY. */
  private readonly owner = new Vector(BigUint64Array, LANE);
  /** When the buffer came home, for the idle release. */
  private readonly home_since = new Vector(Float64Array, LANE);
  private readonly buffers: (ArrayBuffer | null)[] = [];

  /** `bytes` is one job's worth, what a job sends out and brings back;
   * `cap` how many may be away at once, the worker credits; a buffer home for
   * `idle_ms` is released to the collector. The central scheduler's numbers,
   * not the mechanism's. */
  constructor(readonly bytes: u32, readonly cap: u32, readonly idle_ms: f64) {}

  /**
   * A buffer for demand `key`, or null when the cap is reached — the demand
   * waits. A buffer at home is reused; otherwise one is made, up to the cap.
   */
  borrow(key: u64): { id: u32; buffer: ArrayBuffer } | null {
    let id = -1;
    for (let i = 0; i < this.state.len; i++) {
      if (this.state.mem[i] === HOME) {
        id = i;
        break;
      }
    }
    if (id < 0) {
      for (let i = 0; i < this.state.len; i++) {
        if (this.state.mem[i] === NONE) {
          id = i;
          break;
        }
      }
      if (id < 0) {
        if (this.state.len >= this.cap) return null;
        id = this.state.len;
        this.state.push(NONE);
        this.owner.push(0n);
        this.home_since.push(0);
        this.buffers[id] = null;
      }
      this.buffers[id] = new ArrayBuffer(this.bytes);
    }
    const buffer = this.buffers[id]!;
    this.state.mem[id] = AWAY;
    this.owner.mem[id] = key;
    // Away means transferred: the local handle is the detached husk, which
    // is not a buffer. The buffer that comes back is the one that is.
    this.buffers[id] = null;
    return { id, buffer };
  }

  /** The buffer is back from its job, with the buffer that carried it. */
  give_back(id: u32, buffer: ArrayBuffer, now: f64 = performance.now()): void {
    if (this.state.mem[id] !== AWAY) return;
    this.state.mem[id] = HOME;
    this.owner.mem[id] = 0n;
    this.home_since.mem[id] = now;
    this.buffers[id] = buffer;
  }

  /**
   * The job that held the buffer is gone — whoever asked for it no longer
   * wants it, or the worker was lost — and the buffer will not come back
   * through give_back. The slot is freed for a new buffer; whatever the worker
   * still holds is its own to drop.
   */
  forget(key: u64): void {
    for (let i = 0; i < this.state.len; i++) {
      if (this.state.mem[i] === AWAY && this.owner.mem[i] === key) {
        this.state.mem[i] = NONE;
        this.owner.mem[i] = 0n;
        this.buffers[i] = null;
      }
    }
  }

  /** Releases buffers that have sat at home past `idle_ms`. Called from the
   * central scheduler's turn, the way a pool's idle regions are swept. */
  sweep(now: f64 = performance.now()): u32 {
    let released = 0;
    for (let i = 0; i < this.state.len; i++) {
      if (this.state.mem[i] !== HOME || now - this.home_since.mem[i] < this.idle_ms) continue;
      this.state.mem[i] = NONE;
      this.buffers[i] = null;
      released++;
    }
    return released;
  }
}
