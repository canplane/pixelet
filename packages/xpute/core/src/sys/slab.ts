// @xpute/core/sys/slab.ts

/**
 * The little memory: transferable staging buffers for work that leaves the
 * thread.
 *
 * The big memory (memory.ts) is one fixed `WebAssembly.Memory`, and a
 * `WebAssembly.Memory` cannot be transferred to a worker, nor can a window of
 * it. So anything bound for a worker travels in a plain `ArrayBuffer` of its
 * own: a slab, one job's worth, moved by ownership — `postMessage` with the
 * buffer in the transfer list — and moved back the same way when the job is
 * done. That is the whole of the memory model's second half
 * (issue/20260911-single-thread-core-and-when-to-spawn-a-worker.md,
 * "Big and little, decided"), and this is where it lives.
 *
 * Three rules, from that record:
 *
 * - **Fixed-size slabs, each one job's worth.** A transfer moves a whole
 *   buffer, so the slab is the unit of a job; a job that needs more than one
 *   is two jobs.
 * - **Created when first needed, recycled when they come back, released
 *   after sitting idle.** Little buffers have the one freedom big lacks — the
 *   collector can take them back — so an idle one is let go rather than held
 *   for a peak that may not return. Nothing is preallocated: until a worker
 *   exists no slab exists.
 * - **Capped, and the cap is the backpressure.** When every slab is away no
 *   new job starts — and the record of which slabs are away, and for whom, is
 *   the ownership ledger the central scheduler needs: it is what says what to
 *   do with a buffer whose job outlived whoever asked for it.
 *
 * A job borrows a slab, fills it from the big memory, posts it, and on return
 * copies the result back and gives the slab back — the slab away is the
 * request away, and its owner is the job that sent it.
 *
 * Slots are the central scheduler's own lanes — a state and an owner per
 * slab — not the kernel's memory, whose inside is the kernel's; the
 * `ArrayBuffer` handles sit beside them. A borrowed slab's buffer handle is
 * cleared here — after transfer the local buffer is detached anyway, and a
 * detached buffer is not a slab — and set again when it comes back.
 */

import type { f64, u32 } from "../kit/word.ts";
import { Vector } from "../ds/arena.ts";
import { fnv1a64_str } from "../kit/hash.ts";

const NONE = 0;
const HOME = 1;
const AWAY = 2;

const LANE = { init_cap: 8 } as const;

export type SlabStats = { cap: u32; home: u32; away: u32; bytes: u32 };

export class LittleSlabs {
  private readonly state = new Vector(Uint8Array, LANE);
  /** Who has the slab, as the hash of the job's key, while AWAY. */
  private readonly owner = new Vector(BigUint64Array, LANE);
  /** When the slab came home, for the idle release. */
  private readonly home_since = new Vector(Float64Array, LANE);
  private readonly buffers: (ArrayBuffer | null)[] = [];

  /** `bytes` is one job's worth, what a job sends out and brings back;
   * `cap` how many may be away at once, the worker credits; a slab home for
   * `idle_ms` is released to the collector. The central scheduler's numbers,
   * not the mechanism's. */
  constructor(readonly bytes: u32, readonly cap: u32, readonly idle_ms: f64) {}

  /**
   * A slab for `job`, or null when the cap is reached — the job waits. A
   * slab at home is reused; otherwise one is made, up to the cap.
   */
  borrow(job: string): { id: u32; buffer: ArrayBuffer } | null {
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
    this.owner.mem[id] = fnv1a64_str(job);
    // Away means transferred: the local handle is the detached husk, which
    // is not a slab. The buffer that comes back is the one that is.
    this.buffers[id] = null;
    return { id, buffer };
  }

  /** The slab is back from its job, with the buffer that carried it. */
  give_back(id: u32, buffer: ArrayBuffer, now: f64 = performance.now()): void {
    if (this.state.mem[id] !== AWAY) return;
    this.state.mem[id] = HOME;
    this.owner.mem[id] = 0n;
    this.home_since.mem[id] = now;
    this.buffers[id] = buffer;
  }

  /**
   * The job that held the slab is gone — whoever asked for it no longer
   * wants it, or the worker was lost — and the buffer will not come back
   * through give_back. The slot is freed for a new slab; whatever the worker
   * still holds is its own to drop.
   */
  forget(job: string): void {
    const key = fnv1a64_str(job);
    for (let i = 0; i < this.state.len; i++) {
      if (this.state.mem[i] === AWAY && this.owner.mem[i] === key) {
        this.state.mem[i] = NONE;
        this.owner.mem[i] = 0n;
        this.buffers[i] = null;
      }
    }
  }

  /** The job holding slab `id`, or null. */
  owner_of(id: u32): bigint | null {
    return this.state.mem[id] === AWAY ? this.owner.mem[id] : null;
  }

  /** Releases slabs that have sat at home past `idle_ms`. Called from the
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

  stats(): SlabStats {
    let home = 0, away = 0;
    for (let i = 0; i < this.state.len; i++) {
      if (this.state.mem[i] === HOME) home++;
      else if (this.state.mem[i] === AWAY) away++;
    }
    return { cap: this.cap, home, away, bytes: (home + away) * this.bytes };
  }
}
