// @pxl/web/module/world/sched/bake_port.test.ts

import { assert, assertEquals } from "@std/assert";

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { CommandPort } from "@xpute/core/ipc/port.ts";
import { serve, virtual_link_pair } from "@xpute/core/ipc/link.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { bake_chunk_mesh, BakeContext } from "@/render/layer/pipeline/chunk_mesher.ts";
import { CORE_BAKE, release_chunk_mesh } from "@/render/layer/pipeline/core_bake.ts";
import { type MeshRun, pack_column_runs } from "@/render/layer/pipeline/voxel_scan.ts";
import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "@/render/layer/pipeline/voxel.ts";
import { BAKE_CMD, bake_handler } from "@/render/layer/pipeline/bake_command.ts";

import { BakePort, type BakeWorker } from "./bake_port.ts";
import { LittleSlabs } from "./slab.ts";

function run(z0: i32, z1: i32, r: u32, g: u32, b: u32): MeshRun {
  return { z0, z1, r, g, b, layer_idx: NO_LAYER, material_class: MaterialClass.LAND, sub_sz: SUB_CELL_NONE, sub_ox: 0, sub_oy: 0, sub_oz: 0 };
}

function busy_set() {
  const columns: MeshRun[][] = Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []);
  for (let cy = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const h = (cx * 7 + cy * 13) % 5;
      columns[cy * CHUNK_CELL_SPAN + cx].push(run(-3, h, 40 + cx, 80, 40 + cy));
      if ((cx + cy) % 11 === 0) columns[cy * CHUNK_CELL_SPAN + cx].push(run(h + 2, h + 2, 200, 10, 10));
    }
  }
  return pack_column_runs(columns);
}

/** A bake worker on an in-memory link, with a way to kill it. */
function harness(cap: u32 = 4) {
  let spawned = 0;
  let kill: (() => void) | null = null;
  const spawn = (): BakeWorker => {
    spawned++;
    const [near, far] = virtual_link_pair();
    const port = new CommandPort();
    port.on(BAKE_CMD, bake_handler(new BakeContext(new BuddyAllocator({ region_bytes: 1 << 24 }))));
    serve(far, port);
    return { link: near, terminate: () => {}, on_error: (fail) => (kill = fail) };
  };
  const slabs = new LittleSlabs(8 << 20, cap);
  const port = new BakePort(spawn, slabs);
  return { port, slabs, spawned: () => spawned, kill: () => kill?.() };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

Deno.test("bake port - a bake goes away, comes back ready, and adopts into the pool as the same mesh, with its slab home", async () => {
  const { port, slabs } = harness();
  const set = busy_set();
  const tx = port.submit("bake:a", set, 1)!;
  assertEquals(port.poll(tx), "away");
  assertEquals(slabs.stats().away, 1);
  await settle();
  assertEquals(port.poll(tx), "ready");
  assert(port.can_adopt(tx));
  const reply = port.adopt(tx, CORE_BAKE.pool)!;
  const direct = bake_chunk_mesh(CORE_BAKE, set, 1);
  assertEquals(Array.from(reply.mesh.position), Array.from(direct.position));
  assertEquals(Array.from(reply.mesh.index), Array.from(direct.index));
  assertEquals(slabs.stats(), { cap: 4, home: 1, away: 0, bytes: 8 << 20 });
  assertEquals(port.stats.adopted, 1);
  release_chunk_mesh(direct);
  release_chunk_mesh(reply.mesh);
});

Deno.test("bake port - the slab cap is the backpressure: past it a submit waits", () => {
  const { port } = harness(2);
  const set = busy_set();
  assert(port.submit("bake:a", set, 8) !== null);
  assert(port.submit("bake:b", set, 8) !== null);
  assertEquals(port.submit("bake:c", set, 8), null);
});

Deno.test("bake port - a bake cancelled while away is dropped on arrival and its slab goes home", async () => {
  const { port, slabs } = harness();
  const tx = port.submit("bake:a", busy_set(), 4)!;
  port.cancel(tx);
  await settle();
  assertEquals(port.poll(tx), "failed");
  assertEquals(slabs.stats().away, 0);
  assertEquals(port.stats.cancelled, 1);
  assertEquals(port.stats.adopted, 0);
});

Deno.test("bake port - a lost worker fails what it held, writes off its slabs, and the next bake starts another", async () => {
  const { port, slabs, spawned, kill } = harness();
  const logged = console.error;
  console.error = () => {};
  try {
    const tx = port.submit("bake:a", busy_set(), 8)!;
    kill();
    assertEquals(port.poll(tx), "failed");
    assertEquals(slabs.stats().away, 0);
    port.cancel(tx);
    await settle();
    assertEquals(port.poll(tx), "failed", "a reply from the lost worker changes nothing");
    const again = port.submit("bake:a", busy_set(), 8)!;
    assertEquals(spawned(), 2);
    await settle();
    assertEquals(port.poll(again), "ready");
  } finally {
    console.error = logged;
  }
});

Deno.test("bake port - past the tick's upload bytes a second ready mesh waits for the next tick", async () => {
  const { port } = harness();
  const set = busy_set();
  // Step 2 of this chunk is 6.8 MB: one fits a tick, two do not.
  const a = port.submit("bake:a", set, 2)!, b = port.submit("bake:b", set, 2)!;
  await settle();
  port.begin_tick();
  assert(port.can_adopt(a));
  const first = port.adopt(a, CORE_BAKE.pool)!;
  assertEquals(port.can_adopt(b), false);
  assertEquals(port.stats.deferred, 1);
  port.begin_tick();
  assert(port.can_adopt(b));
  const second = port.adopt(b, CORE_BAKE.pool)!;
  release_chunk_mesh(first.mesh);
  release_chunk_mesh(second.mesh);
});
