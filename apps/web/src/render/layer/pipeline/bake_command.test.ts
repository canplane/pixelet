// @pxl/web/render/layer/pipeline/bake_command.test.ts

import { assert, assertEquals } from "@std/assert";

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { Frame, type FrameMessage, make_tx_id } from "@xpute/core/ipc/frame.ts";
import { CommandPort } from "@xpute/core/ipc/port.ts";
import { serve, virtual_link_pair } from "@xpute/core/ipc/link.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import type { LodStep } from "@/render/lod.ts";
import { bake_chunk_mesh, BakeContext, type BakedChunkMesh } from "./chunk_mesher.ts";
import { CORE_BAKE, release_chunk_mesh } from "./core_bake.ts";
import { CORE_POOL } from "./core_pool.ts";
import { type MeshRun, pack_column_runs } from "./voxel_scan.ts";
import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "./voxel.ts";
import { BAKE_CMD, bake_handler, read_bake_reply, write_bake_request } from "./bake_command.ts";

function run(z0: i32, z1: i32, r: u32, g: u32, b: u32): MeshRun {
  return { z0, z1, r, g, b, layer_idx: NO_LAYER, material_class: MaterialClass.LAND, sub_sz: SUB_CELL_NONE, sub_ox: 0, sub_oy: 0, sub_oz: 0 };
}

/** Varied heights and a floating cube every few cells, so every tier has
 * real merge and accumulation decisions to make. */
function busy_columns(): MeshRun[][] {
  const columns: MeshRun[][] = Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []);
  for (let cy = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const h = (cx * 7 + cy * 13) % 5;
      columns[cy * CHUNK_CELL_SPAN + cx].push(run(-3, h, 40 + cx, 80, 40 + cy));
      if ((cx + cy) % 11 === 0) columns[cy * CHUNK_CELL_SPAN + cx].push(run(h + 2, h + 2, 200, 10, 10));
    }
  }
  return columns;
}

function same_bytes(a: BakedChunkMesh, b: BakedChunkMesh): void {
  assertEquals(a.vertex_cnt, b.vertex_cnt);
  for (const key of ["position", "normal", "local_pos", "box_scale", "rgb", "layer_taps", "faces", "index"] as const) {
    assertEquals(new Uint8Array(a[key].buffer, a[key].byteOffset, a[key].byteLength), new Uint8Array(b[key].buffer, b[key].byteOffset, b[key].byteLength), key);
  }
}

/** The far side of a bake worker, on an in-memory link. */
function far_side(): { near: ReturnType<typeof virtual_link_pair>[0]; worker_pool: BuddyAllocator; replies: FrameMessage[] } {
  const [near, far] = virtual_link_pair();
  const worker_pool = new BuddyAllocator({ region_bytes: 1 << 24, max_idle_regions: 1 });
  const port = new CommandPort();
  port.on(BAKE_CMD, bake_handler(new BakeContext(worker_pool)));
  serve(far, port);
  const replies: FrameMessage[] = [];
  near.on((m) => replies.push(m));
  return { near, worker_pool, replies };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

async function bake_across(frame_bytes: u32, step: LodStep) {
  const { near, worker_pool, replies } = far_side();
  const set = pack_column_runs(busy_columns());
  const buf = new ArrayBuffer(frame_bytes);
  const spill = write_bake_request(new Frame(buf), make_tx_id(1, 1), set, step);
  near.send({ frame: buf, spill });
  await settle();
  assertEquals(replies.length, 1);
  const reply = read_bake_reply(new Frame(replies[0].frame), replies[0].spill, CORE_POOL);
  return { set, reply, request_spilled: spill !== null, reply_spilled: replies[0].spill !== null, worker_pool };
}

Deno.test("bake command - a bake sent across the link is byte for byte the bake done in place, at every tier", async () => {
  for (const step of [1, 2, 4, 8] as const) {
    // This chunk is built to defeat merging, so its finer tiers run to 4.7 and
    // 6.8 MB; an 8 MB frame keeps every tier on the in-frame path, and the
    // spill path has its own test below.
    const { set, reply, request_spilled, reply_spilled } = await bake_across(8 << 20, step);
    assert(!request_spilled && !reply_spilled, "an 8 MB frame holds this chunk both ways");
    const occlusion_before = { ...CORE_BAKE.occlusion }, coarse_before = { ...CORE_BAKE.coarse_instances };
    const direct = bake_chunk_mesh(CORE_BAKE, set, step);
    same_bytes(reply.mesh, direct);
    assertEquals(reply.counters, {
      total_runs: CORE_BAKE.occlusion.total_runs - occlusion_before.total_runs,
      culled: CORE_BAKE.occlusion.culled - occlusion_before.culled,
      faces_possible: CORE_BAKE.occlusion.faces_possible - occlusion_before.faces_possible,
      faces_kept: CORE_BAKE.occlusion.faces_kept - occlusion_before.faces_kept,
      coarse_total: CORE_BAKE.coarse_instances.total - coarse_before.total,
      coarse_low_fill: CORE_BAKE.coarse_instances.low_fill - coarse_before.low_fill,
    }, `counters at step ${step}`);
    release_chunk_mesh(direct);
    release_chunk_mesh(reply.mesh);
  }
});

Deno.test("bake command - a frame too small for the run set or the mesh carries them in spills, with the same result", async () => {
  const { set, reply, request_spilled, reply_spilled } = await bake_across(256, 1);
  assert(request_spilled, "the run set went in a spill");
  assert(reply_spilled, "the mesh came back in a spill");
  const direct = bake_chunk_mesh(CORE_BAKE, set, 1);
  same_bytes(reply.mesh, direct);
  release_chunk_mesh(direct);
  release_chunk_mesh(reply.mesh);
});

Deno.test("bake command - the far side keeps nothing: its mesh block is back in its pool once the reply is sent", async () => {
  const { worker_pool, reply } = await bake_across(8 << 20, 1);
  const scratch_bytes = worker_pool.stat.live_bytes;
  release_chunk_mesh(reply.mesh);
  const again = await bake_across(8 << 20, 1);
  assert(scratch_bytes > 0, "the context's scratch lives in the worker's pool");
  assertEquals(again.worker_pool.stat.live_bytes, scratch_bytes, "only the context's scratch stays live between bakes");
  release_chunk_mesh(again.reply.mesh);
});
