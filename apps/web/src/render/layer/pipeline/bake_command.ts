// @pxl/web/render/layer/pipeline/bake_command.ts

/**
 * The chunk bake as a command: what a request carries, what the handler on
 * the far side does with it, and how the reply becomes a mesh again.
 *
 * Request: args are the run set's shape and the tier; the body is the run
 * set's bytes, copied out of the block it lives in. Reply: args are the
 * mesh's counts and what the bake added to the context's counters; the body
 * is the mesh block's bytes. Both sides carve the same layouts
 * (column_run.ts, chunk_mesher.ts#carve_baked_mesh) over whatever buffer the
 * bytes landed in, so a copy is the whole of the translation.
 *
 * Args are fixed words rather than an XTP packet: three numbers in and
 * eight out, where a packet's header and node table would be most of the
 * message. A command whose args are a record takes XTP; this one's are not.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block, BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { Errno } from "@xpute/core/sys/errno.ts";
import { XputeError } from "@xpute/core/sys/error.ts";
import { BODY_IN_SPILL, type Frame, FrameFlag } from "@xpute/core/ipc/frame.ts";
import type { CommandHandler } from "@xpute/core/ipc/port.ts";

import type { LodStep } from "@/render/lod.ts";
import { carve_column_run_set, column_run_set_bytes, type ColumnRunSet } from "./column_run.ts";
import { bake_chunk_mesh, type BakeContext, baked_mesh_bytes, type BakedChunkMesh, carve_baked_mesh } from "./chunk_mesher.ts";

export const BAKE_CMD: u32 = 1;

const REQUEST_ARGS_WORDS: u32 = 3; // count, span, lod_step
const REPLY_ARGS_WORDS: u32 = 8; // vertex_cnt, index_cnt, then the six counter deltas below

/** What one bake added to its context's counters, so the side that keeps
 * the ledger can add the same. */
export type BakeCounters = {
  total_runs: u32;
  culled: u32;
  faces_possible: u32;
  faces_kept: u32;
  coarse_total: u32;
  coarse_low_fill: u32;
};

/** A block view over bytes that belong to no pool — a frame or a spill. */
function view_block(buf: ArrayBuffer, byte_offset: u32, bytes: u32): Block {
  return { buf, byte_offset, bytes, order: -1, region: -1, free: false };
}

function body_buffer(frame: Frame, spill: ArrayBuffer | null): { buf: ArrayBuffer; offset: u32 } {
  if (frame.body_offset !== BODY_IN_SPILL) return { buf: frame.buf, offset: frame.body_offset };
  if (!spill) throw new XputeError(Errno.EBADMSG, "bake: body declared in a spill that did not arrive");
  return { buf: spill, offset: 0 };
}

/**
 * Writes a bake request for `set` at `lod_step` into `frame`. Returns the
 * spill the body had to go into, or null when it fits the frame.
 */
export function write_bake_request(frame: Frame, tx_id: u32, set: ColumnRunSet, lod_step: LodStep): ArrayBuffer | null {
  const body_bytes = column_run_set_bytes(set.count, set.span);
  frame.begin(tx_id, BAKE_CMD, FrameFlag.ACKREQ);
  const at = frame.lay_out(REQUEST_ARGS_WORDS * 4, body_bytes);
  const args = new Uint32Array(frame.buf, frame.args_offset(), REQUEST_ARGS_WORDS);
  args[0] = set.count;
  args[1] = set.span;
  args[2] = lod_step;
  const source = new Uint8Array(set.block.buf, set.block.byte_offset, body_bytes);
  if (at !== BODY_IN_SPILL) {
    new Uint8Array(frame.buf, at, body_bytes).set(source);
    return null;
  }
  const spill = new ArrayBuffer(body_bytes);
  new Uint8Array(spill).set(source);
  return spill;
}

function counters_of(ctx: BakeContext): BakeCounters {
  const o = ctx.occlusion, c = ctx.coarse_instances;
  return { total_runs: o.total_runs, culled: o.culled, faces_possible: o.faces_possible, faces_kept: o.faces_kept, coarse_total: c.total, coarse_low_fill: c.low_fill };
}

/**
 * The handler the far side registers: bakes the request's run set in `ctx`
 * and writes the mesh back into the frame, or into a spill when the frame is
 * too small for it. The mesh's block goes straight back to the context's
 * pool — the reply is a copy.
 */
export function bake_handler(ctx: BakeContext): CommandHandler {
  return (frame, spill) => {
    const args = new Uint32Array(frame.buf, frame.args_offset(), REQUEST_ARGS_WORDS);
    const count = args[0], span = args[1], lod_step = args[2] as LodStep;
    const body = body_buffer(frame, spill);
    const set = carve_column_run_set(view_block(body.buf, body.offset, frame.body_bytes), count, span);

    const before = counters_of(ctx);
    const mesh = bake_chunk_mesh(ctx, set, lod_step);
    const after = counters_of(ctx);

    const index_cnt = mesh.index.length;
    const mesh_bytes = baked_mesh_bytes(mesh.vertex_cnt, index_cnt);
    try {
      const tx_id = frame.tx_id, flags = frame.flags;
      frame.begin(tx_id, BAKE_CMD, flags);
      const at = frame.lay_out(REPLY_ARGS_WORDS * 4, mesh_bytes);
      const out = new Uint32Array(frame.buf, frame.args_offset(), REPLY_ARGS_WORDS);
      out[0] = mesh.vertex_cnt;
      out[1] = index_cnt;
      out[2] = after.total_runs - before.total_runs;
      out[3] = after.culled - before.culled;
      out[4] = after.faces_possible - before.faces_possible;
      out[5] = after.faces_kept - before.faces_kept;
      out[6] = after.coarse_total - before.coarse_total;
      out[7] = after.coarse_low_fill - before.coarse_low_fill;
      const source = new Uint8Array(mesh.block.buf, mesh.block.byte_offset, mesh_bytes);
      if (at !== BODY_IN_SPILL) {
        new Uint8Array(frame.buf, at, mesh_bytes).set(source);
        return null;
      }
      const reply_spill = new ArrayBuffer(mesh_bytes);
      new Uint8Array(reply_spill).set(source);
      return reply_spill;
    } finally {
      ctx.pool.release(mesh.block);
    }
  };
}

export type BakeReply = { mesh: BakedChunkMesh; counters: BakeCounters };

/**
 * Reads a bake reply into `pool`: the mesh's bytes are copied into a block
 * of its own there, so nothing refers to the frame or the spill once this
 * returns and both can go back where they came from. Throws the reply's
 * errno when the far side failed.
 */
export function read_bake_reply(frame: Frame, spill: ArrayBuffer | null, pool: BuddyAllocator): BakeReply {
  if (frame.errno !== 0) throw new XputeError(frame.errno, `bake: far side replied errno ${frame.errno}`);
  const args = new Uint32Array(frame.buf, frame.args_offset(), REPLY_ARGS_WORDS);
  const vertex_cnt = args[0], index_cnt = args[1];
  const counters: BakeCounters = { total_runs: args[2], culled: args[3], faces_possible: args[4], faces_kept: args[5], coarse_total: args[6], coarse_low_fill: args[7] };
  const body = body_buffer(frame, spill);
  const bytes = baked_mesh_bytes(vertex_cnt, index_cnt);
  const block = pool.acquire(bytes);
  new Uint8Array(block.buf, block.byte_offset, bytes).set(new Uint8Array(body.buf, body.offset, bytes));
  return { mesh: carve_baked_mesh(block, vertex_cnt, index_cnt), counters };
}

/** The bytes a ready reply's mesh will take once read — known from its args
 * before anything is copied, so a caller can decide whether to adopt it now. */
export function reply_mesh_bytes(frame: Frame): u32 {
  if (frame.errno !== 0) return 0;
  const args = new Uint32Array(frame.buf, frame.args_offset(), 2);
  return baked_mesh_bytes(args[0], args[1]);
}

/** Adds what a bake done elsewhere counted to `ctx`'s counters, so the
 * ledger reads one total whichever side baked. */
export function add_bake_counters(ctx: BakeContext, counters: BakeCounters): void {
  ctx.occlusion.total_runs += counters.total_runs;
  ctx.occlusion.culled += counters.culled;
  ctx.occlusion.faces_possible += counters.faces_possible;
  ctx.occlusion.faces_kept += counters.faces_kept;
  ctx.coarse_instances.total += counters.coarse_total;
  ctx.coarse_instances.low_fill += counters.coarse_low_fill;
}
