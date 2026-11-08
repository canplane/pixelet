// @xpute/runtime/ipc/head.ts

/**
 * The front's header (spec/ipc/head.json). The runtime lays it in a core's
 * memory from the core's machine before the first turn, as io_uring's setup
 * lays its rings, together with each ring's own header (ring.json). It is
 * read back to find the rings and the GPU stream. On its first turn the core
 * checks the header against its machine.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import { MAGIC, WORDS, word } from "./head.spec.ts";
import * as ring from "./ring.spec.ts";

/** The front a machine declares: the rings' sizes, and where the rings,
 * their slots and the GPU stream lie. `gpu_at` is 0 without the GPU
 * device. */
export interface Front {
  sq_cap: u32;
  cq_cap: u32;
  slot_bytes: u32;
  sq_at: u32;
  cq_at: u32;
  sq_slots_at: u32;
  cq_slots_at: u32;
  gpu_at: u32;
}

/** Whether a header is laid at `at`. A core that outlived its runtime's
 * reload keeps the header laid for it, and its rings' positions with it. */
export function laid(mem: ArrayBufferLike, at: u32): boolean {
  return new Uint32Array(mem, at, WORDS)[word.MAGIC] === MAGIC;
}

/** Lays the header at `at`, and each ring's header with its positions at
 * 0. */
export function lay(mem: ArrayBufferLike, at: u32, f: Front): void {
  const h = new Uint32Array(mem, at, WORDS);
  h[word.MAGIC] = MAGIC;
  h[word.SQ_CAP] = f.sq_cap;
  h[word.CQ_CAP] = f.cq_cap;
  h[word.SLOT_BYTES] = f.slot_bytes;
  h[word.SQ_AT] = f.sq_at;
  h[word.CQ_AT] = f.cq_at;
  h[word.GPU_AT] = f.gpu_at;
  for (const [ring_at, cap, slots_at] of [
    [f.sq_at, f.sq_cap, f.sq_slots_at],
    [f.cq_at, f.cq_cap, f.cq_slots_at],
  ]) {
    const r = new Uint32Array(mem, ring_at, ring.HEADER_WORDS);
    r.fill(0);
    r[ring.CAP] = cap;
    r[ring.SLOT_BYTES] = f.slot_bytes;
    r[ring.SLOT_BASE] = slots_at;
  }
}

/** Where a laid header says the rings and the GPU stream are. A header
 * without xpute's MAGIC throws EBADMSG. */
export class Head {
  readonly sq_at: u32;
  readonly cq_at: u32;
  readonly gpu_at: u32;

  constructor(mem: ArrayBufferLike, at: u32) {
    const h = new Uint32Array(mem, at, WORDS);
    if (h[word.MAGIC] !== MAGIC) throw new MarshalError(Errno.EBADMSG);
    this.sq_at = h[word.SQ_AT];
    this.cq_at = h[word.CQ_AT];
    this.gpu_at = h[word.GPU_AT];
  }
}
