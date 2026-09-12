// @pxl/web/modules/world/chunk_block.test.ts

import { assertEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { WorldLayer } from "@/render/layer/pipeline/contract.ts";
import { fill_chunk_column, MaterialClass, NO_LAYER } from "@/render/layer/pipeline/volume.ts";
import { column_scan_open } from "@/render/layer/pipeline/voxel_scan.ts";
import { block_count, block_for, block_key_of, block_member_index, block_release, block_reveal_region, block_unit_of, step_uses_block } from "./chunk_block.ts";

const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;
const page = ivec.gather(7n, 3n, 0n);
const key_at = (cx: number, cy: number) => ({ page_idx: page, page_off: ivec.gather(BigInt(cx * 64), BigInt(cy * 64), 0n) });

/** Paints one 16-cell-tall pillar per chunk, at a column that names the chunk. */
const per_chunk: WorldLayer = {
  name: "test-per-chunk",
  revision: () => [1],
  paint: (key, target) => {
    const cx = Number(ivec.X(key.page_off) / 64n), cy = Number(ivec.Y(key.page_off) / 64n);
    // Chunk (1, 1) of the unit is left empty.
    if (cx === 1 && cy === 1) return false;
    fill_chunk_column(target, 8 * cx, 8 * cy, 0, 15, LAND);
    return true;
  },
};

Deno.test("block - units follow the step, and a chunk knows its place in one", () => {
  assertEquals([step_uses_block(4), step_uses_block(8), step_uses_block(16)], [false, true, true]);
  assertEquals([block_unit_of(4), block_unit_of(8), block_unit_of(16)], [1, 2, 4]);
  // Steps the range never shows still name a unit the scan can hold.
  assertEquals([block_unit_of(32), block_unit_of(64)], [4, 4]);
  assertEquals(block_key_of(key_at(3, 2), 16).page_off, key_at(0, 0).page_off);
  assertEquals(block_key_of(key_at(5, 2), 16).page_off, key_at(4, 0).page_off);
  assertEquals(block_member_index(key_at(5, 2), 16), { ux: 1, uy: 2 });
  assertEquals(block_member_index(key_at(3, 1), 8), { ux: 1, uy: 1 });
});

Deno.test("block - a 2x2 unit bakes once at step 8 and each member reveals only its own region", () => {
  const before = block_count();
  const members = [key_at(0, 0), key_at(1, 0), key_at(0, 1), key_at(1, 1)];
  const blocks = members.map((k) => block_for(k, 8, [per_chunk]));
  assertEquals(new Set(blocks).size, 1, "four members share one block");
  assertEquals(block_count(), before + 1);
  const block = blocks[0];

  // Drive it to completion with no budget, as a source-change rebuild does.
  let state: string;
  do state = block.advance(8, null); while (state === "busy");
  assertEquals(state, "done");
  const bucket = block.bucket_at(8)!;
  assertEquals(bucket.lod_step, 8);
  // Adopted per region, so it starts entirely hidden.
  assertEquals(Array.from(bucket.cubes.visibility).every((v) => v === 0), true);

  // Member (1, 0) reveals its region: some vertices become visible, and every
  // one of them lies in that region by box centre.
  block_reveal_region(bucket, 1, 0, 1);
  const { position, local_pos, box_scale, visibility, vertex_cnt } = bucket.cubes;
  let shown = 0;
  for (let v = 0; v < vertex_cnt; v++) {
    if (visibility[v] !== 1) continue;
    shown++;
    const cx = position[v * 3] - local_pos[v * 3] * box_scale[v * 3];
    const cy = position[v * 3 + 1] - local_pos[v * 3 + 1] * box_scale[v * 3 + 1];
    assertEquals(cx >= 64 && cx < 128 && cy >= 0 && cy < 64, true, `vertex ${v} centre (${cx}, ${cy}) outside region (1, 0)`);
  }
  assertEquals(shown > 0, true);
  // The empty member (1, 1) has nothing to reveal.
  const before_empty = Array.from(visibility);
  block_reveal_region(bucket, 1, 1, 1);
  assertEquals(Array.from(visibility), before_empty);

  for (const b of blocks) block_release(b);
  assertEquals(block_count(), before, "the last member's release drops the block");
});

Deno.test("block - a member leaving mid-scan gives the scan slot back, even while others remain", () => {
  // The shape three deadlocks took in one afternoon: a block is advanced by
  // whichever member commits, the driving member leaves (refines, is
  // released, goes off screen), and the scan it started holds the one slot
  // for the whole world with nobody left to advance it.
  const a = block_for(key_at(0, 0), 8, [per_chunk]);
  const b = block_for(key_at(1, 0), 8, [per_chunk]);
  assertEquals(a, b);
  const one_step = { remaining: () => 0 } as unknown as import("@/modules/world/sched/tick.ts").FrameBudget;
  assertEquals(a.advance(8, one_step), "busy");
  assertEquals(column_scan_open(), true, "the block holds the scan slot mid-scan");
  assertEquals(a.holds_slot, true);

  block_release(a); // one member leaves; the other remains
  assertEquals(column_scan_open(), false, "the slot is free for the world");
  assertEquals(b.holds_slot, false);
  // The remaining member can still drive the block to completion.
  let state: string;
  do state = b.advance(8, null); while (state === "busy");
  assertEquals(state, "done");
  block_release(b);
});
