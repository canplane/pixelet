// @pxl/web/render/layer/pipeline/wasm_bake.test.ts

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";
import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { LOD_STEP_BY_LEVEL } from "@/render/lod.ts";
import { instantiate_pxl, type PxlExports } from "@/wasm/pxl.ts";
import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "./voxel.ts";
import { type MeshRun, pack_column_runs, release_column_runs } from "./voxel_scan.ts";
import { bake_chunk_mesh, BakeContext, type BakedChunkMesh, carve_visibility } from "./chunk_mesher.ts";
import { CORE_MEMORY, CORE_POOL, MODULE_SECTION } from "./core_pool.ts";
import { carve_column_run_set } from "./column_run.ts";
import { type MeshPool, WasmBaker } from "./wasm_bake.ts";

/** The module over the core memory, which is where pack_column_runs's sets
 * and the mesh pool's blocks are. Instantiated per test: each instance
 * starts the module's statics afresh. */
function pxl(): Promise<PxlExports> {
  assert(CORE_MEMORY !== null, "the core memory reserved");
  return instantiate_pxl(CORE_MEMORY);
}

/** A context whose meshes come from the core pool, as the wasm bake's
 * must, with scratch of its own so the two contexts under test share
 * nothing but the pool. */
function core_context(): BakeContext {
  return new BakeContext(CORE_POOL, buddy_alloc(new BuddyAllocator()));
}

/** A seeded xorshift, so a failure names a scene that can be rebuilt. */
function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

/**
 * Columns of stacked runs with gaps, objects among the terrain, sub-cell
 * boxes, and layers: every branch the face mask, the merge and the coarse
 * accumulation take. `flat` leaves long equal stretches so the merge has
 * wide rectangles to find rather than single cells.
 *
 * Out of order the way the scan writes them, too: a band's overflow
 * sub-voxels follow its voxel runs (voxel_scan.ts, RunSink), so here a
 * sub-cell box is echoed again a run or two later, below runs already
 * written — the order the neighbour scans' early stops must survive.
 */
function scene(seed: u32, flat: boolean): MeshRun[][] {
  const next = rng(seed);
  const columns: MeshRun[][] = Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []);
  for (let cy = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const runs = columns[cy * CHUNK_CELL_SPAN + cx];
      const band = flat ? ((cx >> 4) + (cy >> 3)) % 3 : next() % 7;
      let z = -8 + band;
      const stack = 1 + (flat ? (cx % 9 === 0 ? 1 : 0) : next() % 4);
      const overflow: MeshRun[] = [];
      for (let k = 0; k < stack; k++) {
        const kind = flat ? (k === 0 ? 0 : 1) : next() % 6;
        const height = flat ? 3 : next() % 5;
        const colour = flat ? 90 : next() % 256;
        const layer = flat ? 3 : next() % 3 === 0 ? NO_LAYER : next() % 12;
        const run: MeshRun = {
          z0: z,
          z1: z + (kind === 4 ? 0 : height),
          r: colour,
          g: flat ? 140 : next() % 256,
          b: flat ? 60 : next() % 256,
          layer_idx: layer,
          material_class: kind === 1 || kind === 3 ? MaterialClass.OBJECT : MaterialClass.LAND,
          sub_sz: kind === 4 ? 1 + (next() % 255) : SUB_CELL_NONE,
          sub_ox: kind === 4 ? next() % 256 : 0,
          sub_oy: kind === 4 ? next() % 256 : 0,
          sub_oz: kind === 4 ? next() % 256 : 0,
        };
        runs.push(run);
        if (kind === 4 && next() % 2 === 0) overflow.push({ ...run, sub_sz: 1 + (next() % 255), sub_ox: next() % 256 });
        if (overflow.length > 0 && next() % 2 === 0) runs.push(...overflow.splice(0));
        z = run.z1 + 1 + (flat ? 0 : next() % 3);
      }
      runs.push(...overflow);
    }
  }
  return columns;
}

/** Every lane the bake writes, as bytes — the visibility lane is left out:
 * the bake does not write it, and a pool block carries whatever was there. */
function written_bytes(mesh: BakedChunkMesh): Uint8Array[] {
  const vis = carve_visibility(mesh);
  const block = mesh.block;
  return [
    new Uint8Array(block.buf, block.byte_offset, vis.byteOffset - block.byte_offset),
    new Uint8Array(mesh.index.buffer, mesh.index.byteOffset, mesh.index.byteLength),
  ];
}

function first_difference(a: Uint8Array, b: Uint8Array): number {
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return a.length === b.length ? -1 : a.length;
}

Deno.test("wasm bake - the Rust mesher bakes the TypeScript mesher's mesh byte for byte, at every tier", async () => {
  const ts_ctx = core_context();
  const wasm_ctx = core_context();
  const baker = new WasmBaker(await pxl(), CORE_POOL, wasm_ctx);

  for (const [seed, flat] of [[1, true], [2, false], [3, false], [0x5eed, false]] as const) {
    const set = pack_column_runs(scene(seed, flat));
    for (const step of LOD_STEP_BY_LEVEL) {
      const expected = bake_chunk_mesh(ts_ctx, set, step);

      const cursor = baker.begin(set, step);
      assert(cursor !== null);
      let steps = 0;
      while (!baker.step(cursor)) steps++;
      const actual = baker.finish(cursor);

      const at = `scene ${seed} at step ${step}`;
      assert(expected.vertex_cnt > 0, `${at} baked nothing, which proves nothing`);
      assert(steps > 2, `${at} took only ${steps} steps`);
      assertEquals(actual.vertex_cnt, expected.vertex_cnt, `${at}: vertex count`);
      assertEquals(actual.index.length, expected.index.length, `${at}: index count`);
      const [a, b] = [written_bytes(actual), written_bytes(expected)];
      for (let k = 0; k < a.length; k++) {
        assertEquals(first_difference(a[k], b[k]), -1, `${at}: ${k === 0 ? "vertex" : "index"} bytes differ`);
      }
      assertEquals(wasm_ctx.occlusion, ts_ctx.occlusion, `${at}: occlusion counters`);
      assertEquals(wasm_ctx.coarse_instances, ts_ctx.coarse_instances, `${at}: coarse counters`);

      CORE_POOL.release(expected.block);
      CORE_POOL.release(actual.block);
    }
    release_column_runs(set);
  }
});

Deno.test("wasm bake - a pool that refuses the mesh starves the bake, which finishes once there is room", async () => {
  // The core pool, refusing until told otherwise: the bake must still get a
  // block in the module's memory once it does.
  let refuse = true;
  const pool: MeshPool = { try_acquire: (bytes) => refuse ? null : CORE_POOL.try_acquire(bytes), release: (block) => CORE_POOL.release(block) };
  const ctx = core_context();
  const baker = new WasmBaker(await pxl(), pool, ctx);
  const set = pack_column_runs(scene(7, false));

  const cursor = baker.begin(set, 8)!;
  while (!baker.step(cursor) && !cursor.starved) { /* until the mesh is asked for */ }
  assert(cursor.starved && !cursor.done);
  const kept = ctx.occlusion.faces_kept;
  assert(!baker.step(cursor) && cursor.starved, "still starved on the next step");
  assertEquals(baker.begin(set, 1), null, "a starved bake still holds the slot");
  refuse = false;
  while (!baker.step(cursor)) assert(!cursor.starved);
  const mesh = baker.finish(cursor);
  assert(ctx.occlusion.faces_kept > kept, "the counters arrive with the mesh");

  const direct = bake_chunk_mesh(core_context(), set, 8);
  assertEquals(mesh.vertex_cnt, direct.vertex_cnt);
  const [a, b] = [written_bytes(mesh), written_bytes(direct)];
  assertEquals(first_difference(a[0], b[0]), -1);
  CORE_POOL.release(mesh.block);
  CORE_POOL.release(direct.block);
  release_column_runs(set);
});

Deno.test("wasm bake - an abort at any point leaves the next bake as it would have been", async () => {
  const ctx = core_context();
  const baker = new WasmBaker(await pxl(), CORE_POOL, ctx);
  const set = pack_column_runs(scene(11, false));

  const bake = (step: 1 | 4): BakedChunkMesh => {
    const c = baker.begin(set, step)!;
    while (!baker.step(c)) { /* whole */ }
    return baker.finish(c);
  };
  const before = [bake(1), bake(4)];
  for (const step of [1, 4] as const) {
    for (const after_steps of [0, 1, 17]) {
      const c = baker.begin(set, step)!;
      for (let i = 0; i < after_steps; i++) baker.step(c);
      baker.abort(c);
      assert(!baker.is_open());
    }
  }
  const after = [bake(1), bake(4)];
  for (let k = 0; k < 2; k++) {
    const [a, b] = [written_bytes(after[k]), written_bytes(before[k])];
    assertEquals(first_difference(a[0], b[0]), -1);
    assertEquals(first_difference(a[1], b[1]), -1);
  }
  for (const m of [...before, ...after]) CORE_POOL.release(m.block);
  release_column_runs(set);
});

/** A dense chunk: every column a stack of eight runs with air between,
 * 32k runs in all — about the most a chunk can hold and still have its
 * fine mesh fit a pool region. The coarse bake holding every cell's
 * buckets at once ran the module's heap out on such a chunk (b3). */
function dense_scene(): MeshRun[][] {
  const next = rng(0xdead);
  const columns: MeshRun[][] = Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []);
  for (const runs of columns) {
    let z = -60 + (next() % 8);
    for (let k = 0; k < 8; k++) {
      const h = 1 + (next() % 4);
      runs.push({
        z0: z,
        z1: z + h,
        r: next() % 256,
        g: next() % 256,
        b: next() % 256,
        layer_idx: next() % 12,
        material_class: MaterialClass.LAND,
        sub_sz: SUB_CELL_NONE,
        sub_ox: 0,
        sub_oy: 0,
        sub_oz: 0,
      });
      z += h + 2 + (next() % 3);
    }
  }
  return columns;
}

Deno.test("wasm bake - a dense chunk at every tier stays well inside the module's heap", async () => {
  const ctx = core_context();
  const baker = new WasmBaker(await pxl(), CORE_POOL, ctx);
  const set = pack_column_runs(dense_scene());
  assertEquals(set.count, 8 * CHUNK_CELL_SPAN * CHUNK_CELL_SPAN);
  const ts_ctx = core_context();
  for (const step of LOD_STEP_BY_LEVEL) {
    const expected = bake_chunk_mesh(ts_ctx, set, step);
    const c = baker.begin(set, step)!;
    while (!baker.step(c)) assert(!c.starved && !c.spilled);
    const actual = baker.finish(c);
    assertEquals(first_difference(written_bytes(actual)[0], written_bytes(expected)[0]), -1, `step ${step}`);
    assert(baker.heap_used < MODULE_SECTION.bytes / 4, `step ${step}: module heap at ${(baker.heap_used / 1048576).toFixed(1)} MB`);
    CORE_POOL.release(actual.block);
    CORE_POOL.release(expected.block);
  }
  release_column_runs(set);
});

Deno.test("wasm bake - the module keeps what the map reserves for it, and its heap stays inside", async () => {
  const exports = await pxl();
  assertEquals(exports.pxl_module_bytes(), MODULE_SECTION.bytes);
  const ctx = core_context();
  const baker = new WasmBaker(exports, CORE_POOL, ctx);
  const set = pack_column_runs(scene(2, false));
  for (const step of [1, 2] as const) {
    const c = baker.begin(set, step)!;
    while (!baker.step(c)) { /* whole */ }
    CORE_POOL.release(baker.finish(c).block);
  }
  assert(baker.heap_used > 0 && baker.heap_used < MODULE_SECTION.bytes / 4, `module heap at ${baker.heap_used} bytes`);
  release_column_runs(set);
});

Deno.test("wasm bake - a block the module cannot reach takes a staged mesh, or is handed back when the mesh is too large to stage", async () => {
  // A pool that spills: its blocks are in a buffer of their own, as the
  // mesh section's are past its range.
  const spill = new BuddyAllocator();
  let released = 0;
  const pool: MeshPool = { try_acquire: (bytes) => spill.try_acquire(bytes), release: (block) => (released++, spill.release(block)) };
  const ctx = core_context();
  const baker = new WasmBaker(await pxl(), pool, ctx);
  const set = pack_column_runs(scene(5, false));

  // Step 4's mesh fits the stage: baked in the module, copied out, the same bytes.
  const expected = bake_chunk_mesh(core_context(), set, 4);
  const staged = baker.begin(set, 4)!;
  while (!baker.step(staged)) assert(!staged.starved && !staged.spilled);
  const mesh = baker.finish(staged);
  assertEquals(baker.staged, 1);
  assert(mesh.block.buf !== CORE_MEMORY!.buffer, "the mesh is in the spilled block");
  assertEquals(first_difference(written_bytes(mesh)[0], written_bytes(expected)[0]), -1);
  spill.release(mesh.block);
  CORE_POOL.release(expected.block);

  // Step 1's is 16 MB, past the stage: the block goes straight back.
  const cursor = baker.begin(set, 1)!;
  while (!baker.step(cursor) && !cursor.spilled) assert(!cursor.starved);
  assert(cursor.spilled && !cursor.done);
  assertEquals(released, 1, "the unreachable block went straight back");
  baker.abort(cursor);
  assert(!baker.is_open());
  release_column_runs(set);
});

Deno.test("core bake - a spilled bake finishes the TypeScript way with the same mesh, and a spilled set never enters the module", async () => {
  const { begin_core_bake } = await import("./core_bake.ts");
  const set = pack_column_runs(scene(6, false));
  // The module loads on the first ask; wait for it.
  let job = begin_core_bake("wasm", set, 4);
  for (let i = 0; job === null && i < 100; i++) {
    await new Promise((r) => setTimeout(r, 10));
    job = begin_core_bake("wasm", set, 4);
  }
  assert(job !== null && job.mesher === "wasm");
  while (!job.advance()) assert(!job.starved());
  const through_module = job.finish();

  // The same set in a spilled block: baked the TypeScript way from the start.
  const spilled = new BuddyAllocator().acquire(set.block.bytes);
  new Uint8Array(spilled.buf, spilled.byte_offset, set.block.bytes).set(new Uint8Array(set.block.buf, set.block.byte_offset, set.block.bytes));
  const outside = carve_column_run_set(spilled, set.count, set.span);
  const ts_job = begin_core_bake("wasm", outside, 4)!;
  assert(ts_job !== null);
  while (!ts_job.advance()) { /* whole */ }
  const through_ts = ts_job.finish();

  assertEquals(through_ts.vertex_cnt, through_module.vertex_cnt);
  const [a, b] = [written_bytes(through_ts), written_bytes(through_module)];
  assertEquals(first_difference(a[0], b[0]), -1);
  CORE_POOL.release(through_module.block);
  CORE_POOL.release(through_ts.block);
  release_column_runs(set);
});

Deno.test("core bake - a mesh section that spills mid-bake finishes the TypeScript way with the same mesh", async () => {
  const { begin_core_bake, CORE_BAKE } = await import("./core_bake.ts");
  const { bake_chunk_mesh: whole } = await import("./chunk_mesher.ts");
  const set = pack_column_runs(scene(8, false));
  const expected = whole(CORE_BAKE, set, 1);
  // Use the mesh section up, so the next block spills to a buffer of its own.
  const hogs: Block[] = [];
  for (;;) {
    const block = CORE_POOL.acquire(1 << 20);
    if (block.buf !== CORE_MEMORY!.buffer) {
      CORE_POOL.release(block);
      break;
    }
    hogs.push(block);
  }
  try {
    let job = begin_core_bake("wasm", set, 1);
    for (let i = 0; job === null && i < 100; i++) {
      await new Promise((r) => setTimeout(r, 10));
      job = begin_core_bake("wasm", set, 1);
    }
    assert(job !== null);
    let steps = 0;
    while (!job.advance()) {
      assert(!job.starved());
      assert(++steps < 10000);
    }
    const actual = job.finish();
    assertEquals(actual.vertex_cnt, expected.vertex_cnt);
    assertEquals(first_difference(written_bytes(actual)[0], written_bytes(expected)[0]), -1);
    CORE_POOL.release(actual.block);
  } finally {
    for (const h of hogs) CORE_POOL.release(h);
    CORE_POOL.release(expected.block);
    release_column_runs(set);
  }
});

Deno.test("core bake - a chunk of nothing bakes to an empty mesh through the module", async () => {
  const { begin_core_bake } = await import("./core_bake.ts");
  const set = pack_column_runs(Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []));
  for (const step of [1, 4] as const) {
    let job = begin_core_bake("wasm", set, step);
    for (let i = 0; job === null && i < 100; i++) {
      await new Promise((r) => setTimeout(r, 10));
      job = begin_core_bake("wasm", set, step);
    }
    assert(job !== null);
    let steps = 0;
    while (!job.advance()) assert(++steps < 100 && !job.starved());
    const mesh = job.finish();
    assertEquals(mesh.vertex_cnt, 0);
    CORE_POOL.release(mesh.block);
  }
  release_column_runs(set);
});

Deno.test("core bake - many chunks interleaving, aborting and spilling never leave the module in a state a step trips on", async () => {
  const { begin_core_bake, begin_core_scan } = await import("./core_bake.ts");
  const { alloc_chunk_volume, fill_chunk_column, release_chunk_volume, seal_chunk_volume } = await import("./volume.ts");
  const next = rng(99);
  const sets = [1, 2, 3].map((seed) => pack_column_runs(scene(seed, seed === 1)));
  const empty = pack_column_runs(Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []));
  const volume = () => {
    const v = alloc_chunk_volume();
    for (let c = 0; c < 64; c++) fill_chunk_column(v, c, c, -3, next() % 10, { material_class: MaterialClass.LAND, r: 1, g: 2, b: 3, layer_idx: NO_LAYER });
    return seal_chunk_volume(v);
  };
  // Wait for the module.
  for (let i = 0; i < 100; i++) {
    const j = begin_core_bake("wasm", empty, 1);
    if (j) {
      j.abort();
      break;
    }
    await new Promise((r) => setTimeout(r, 10));
  }
  const hogs: Block[] = [];
  type Live = { kind: "bake" | "scan"; job: { advance(): boolean; starved(): boolean; finish(): unknown; abort(): void }; volume?: ReturnType<typeof volume> };
  const live: Live[] = [];
  for (let round = 0; round < 400; round++) {
    // Sometimes fill the section so blocks spill, sometimes free it.
    if (next() % 40 === 0) {
      if (hogs.length === 0) {
        for (;;) {
          const b = CORE_POOL.acquire(1 << 20);
          if (b.buf !== CORE_MEMORY!.buffer) {
            CORE_POOL.release(b);
            break;
          }
          hogs.push(b);
        }
      } else {
        for (const h of hogs.splice(0)) CORE_POOL.release(h);
      }
    }
    if (live.length < 3 && next() % 2 === 0) {
      if (next() % 2 === 0) {
        const set = next() % 5 === 0 ? empty : sets[next() % 3];
        const job = begin_core_bake("wasm", set, [1, 2, 4, 8][next() % 4] as 1 | 2 | 4 | 8);
        if (job) live.push({ kind: "bake", job });
      } else {
        const v = volume();
        const job = begin_core_scan("wasm", v);
        if (job) live.push({ kind: "scan", job, volume: v });
        else release_chunk_volume(v);
      }
    }
    if (live.length === 0) continue;
    const i = next() % live.length;
    const l = live[i];
    const r = next() % 10;
    if (r === 0) {
      l.job.abort();
      if (l.volume) release_chunk_volume(l.volume);
      live.splice(i, 1);
      continue;
    }
    const done = l.job.advance();
    if (done) {
      const out = l.job.finish() as { block: Block };
      CORE_POOL.release(out.block);
      if (l.volume) release_chunk_volume(l.volume);
      live.splice(i, 1);
    }
  }
  for (const l of live) {
    l.job.abort();
    if (l.volume) release_chunk_volume(l.volume);
  }
  for (const h of hogs) CORE_POOL.release(h);
  for (const s of [...sets, empty]) release_column_runs(s);
});
