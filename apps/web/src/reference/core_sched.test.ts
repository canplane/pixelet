// @pxl/web/reference/core_sched.test.ts

/**
 * The module's scheduler through the glue's channel: a chunk added with a
 * page and a frame written, ticks until its layout lands — every tier of it
 * handed over first — the blend and visibility it is told, a camera far
 * away releasing it, and a retain budget of nothing giving its tiers back.
 */

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { LodStep } from "@/module/world/lod.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { in_range_chunks } from "@/module/world/frame/range.ts";
import { page } from "@/reference/page_fixture.ts";
import "@/reference/terrain_layer.ts";
import "@/reference/object_layer.ts";
import type { BakedChunkMesh } from "@/reference/chunk_mesher.ts";
import { CORE_BUILD, FRAME, release_chunk_mesh } from "@/wasm/core_build.ts";
import { decor_block_release, type DecorBlock } from "@/render/mesh/decor_block.ts";

const terrain = page(33);

type Heard = {
  tiers: Map<LodStep, BakedChunkMesh>;
  layouts: { primary: LodStep; secondary: LodStep | null; ready: boolean }[];
  visible: boolean[];
  evicted: LodStep[];
};

function chunk(x: u32, y: u32): { id: u32; heard: Heard } {
  const heard: Heard = { tiers: new Map(), layouts: [], visible: [], evicted: [] };
  const sink = {
    on_tier: (_g: u32, step: LodStep, mesh: BakedChunkMesh) => heard.tiers.set(step, mesh),
    on_runs: () => {},
    on_visible: (on: boolean) => heard.visible.push(on),
    on_layout: (primary: LodStep, secondary: LodStep | null, ready: boolean) => {
      if (ready) {
        assert(heard.tiers.has(primary), `layout ${primary} before its tier`);
        if (secondary !== null) assert(heard.tiers.has(secondary), `layout ${secondary} before its tier`);
      }
      heard.layouts.push({ primary, secondary, ready });
    },
    on_blend: () => {},
    on_evict: (step: LodStep) => {
      heard.evicted.push(step);
      const mesh = heard.tiers.get(step);
      if (mesh) release_chunk_mesh(mesh);
      heard.tiers.delete(step);
    },
    on_decor: (_s: LodStep, d: DecorBlock) => decor_block_release(d),
  };
  const id = CORE_BUILD.add(sink, ivec.gather(BigInt(x), BigInt(y), 0n), x, y, x, y, 7);
  CORE_BUILD.source(id, terrain, false);
  return { id, heard };
}

/** A frame with the camera at (cx, cy, cz), an identity root, and a frustum
 * that keeps everything. */
function frame(cx: number, cy: number, cz: number, retain_budget: number = 1 << 30): void {
  const f = CORE_BUILD.frame;
  f.fill(0);
  for (const k of [0, 5, 10, 15]) f[FRAME.root + k] = 1;
  for (let p = 0; p < 6; p++) f[FRAME.planes + p * 4 + 3] = 1;
  f[FRAME.camera] = cx;
  f[FRAME.camera + 1] = cy;
  f[FRAME.camera + 2] = cz;
  f[FRAME.lead] = cx;
  f[FRAME.lead + 1] = cy;
  f[FRAME.lead + 2] = cz;
  f[FRAME.viewport_h] = 1080;
  f[FRAME.fov_deg] = 50;
  f[FRAME.delta_s] = 1;
  f[FRAME.budget_ms] = 1000;
  f[FRAME.hard_slack_ms] = 4;
  f[FRAME.retain_budget_bytes] = retain_budget;
}

function tick_until(done: () => boolean, max: number = 50): void {
  for (let i = 0; i < max && !done(); i++) CORE_BUILD.tick();
  assert(done(), "the scheduler never got there");
}

Deno.test("core sched - a chunk on screen lands its layout with its tiers handed over first, and a far camera releases it", () => {
  const { id, heard } = chunk(320, 448);
  // Standing over the chunk: the finest tier.
  frame(352, 480, 60);
  tick_until(() => heard.layouts.some((l) => l.ready));
  assertEquals(heard.visible[0], true);
  const shown = heard.layouts.find((l) => l.ready)!;
  assertEquals(shown.primary, 1);

  // Past the render range the chunk is off screen: a camera that cannot
  // see it (a plane that rejects everything) releases it.
  frame(352, 480, 60);
  CORE_BUILD.frame[FRAME.planes + 3] = -1e9;
  CORE_BUILD.frame[FRAME.camera] = 1e6;
  tick_until(() => heard.layouts.at(-1)?.ready === false);
  assertEquals(heard.visible.at(-1), false);

  // Nothing may be kept off screen: its tiers are given back.
  CORE_BUILD.frame[FRAME.retain_budget_bytes] = 0;
  tick_until(() => heard.tiers.size === 0);
  assert(heard.evicted.length > 0);

  CORE_BUILD.remove(id);
  CORE_BUILD.pump();
  for (const mesh of heard.tiers.values()) release_chunk_mesh(mesh);
});

Deno.test("core sched - a coarse camera lands a coarse tier, and build_now rebuilds the layout on the spot", () => {
  const { id, heard } = chunk(0, 0);
  frame(32, 32, 2000);
  tick_until(() => heard.layouts.some((l) => l.ready));
  const coarse = heard.layouts.find((l) => l.ready)!;
  assert(coarse.primary >= 8, `a coarse tier far away, not ${coarse.primary}`);

  // A source change and the rebuild in the same call.
  for (const mesh of heard.tiers.values()) release_chunk_mesh(mesh);
  heard.tiers.clear();
  const before = heard.layouts.length;
  CORE_BUILD.source(id, terrain, true);
  CORE_BUILD.build_now(id);
  assertEquals(heard.layouts.length, before + 1, "the layout lands in the same call");
  assert(heard.layouts.at(-1)!.ready);

  CORE_BUILD.remove(id);
  CORE_BUILD.pump();
  for (const mesh of heard.tiers.values()) release_chunk_mesh(mesh);
});

Deno.test("core sched - past the retain budget, the farthest off-screen chunk gives its tiers back first", () => {
  const near = chunk(320, 448);
  const far = chunk(960, 960);
  frame(352, 480, 60);
  tick_until(() => near.heard.layouts.some((l) => l.ready) && far.heard.layouts.some((l) => l.ready), 200);

  // Both off screen, the camera still by the near one.
  CORE_BUILD.frame[FRAME.planes + 3] = -1e9;
  CORE_BUILD.frame[FRAME.camera] = -1e6;
  tick_until(() => near.heard.layouts.at(-1)?.ready === false && far.heard.layouts.at(-1)?.ready === false, 200);
  // Room for all but a little: one chunk's worth freed is enough.
  CORE_BUILD.frame[FRAME.retain_budget_bytes] = CORE_BUILD.census().retained_off - 1;
  CORE_BUILD.tick();
  assert(far.heard.evicted.length > 0, "the far chunk gave its tiers back");
  assertEquals(near.heard.evicted.length, 0, "the near one kept them");

  for (const c of [near, far]) {
    CORE_BUILD.remove(c.id);
    for (const mesh of c.heard.tiers.values()) release_chunk_mesh(mesh);
  }
  CORE_BUILD.pump();
});

Deno.test("core sched - the module mounts the chunks of a tiled page within the range, keeps them past it by the hysteresis, and unmounts the rest", () => {
  const mounted = new Map<u32, [u32, u32]>();
  const unmounted: u32[] = [];
  CORE_BUILD.mounts = {
    on_mount: (id, _page, bx, by) => {
      mounted.set(id, [bx, by]);
      CORE_BUILD.attach(id, {
        on_tier: (_g, _s, m) => release_chunk_mesh(m),
        on_runs: () => {},
        on_visible: () => {},
        on_layout: () => {},
        on_blend: () => {},
        on_evict: () => {},
        on_decor: (_s, d) => decor_block_release(d),
      });
    },
    on_unmount: (id) => {
      mounted.delete(id);
      unmounted.push(id);
    },
  };
  const PAGE = ivec.gather(3n, 4n, 0n);
  const page_x = 3 * 1024, page_y = 4 * 1024;
  CORE_BUILD.page(PAGE, true);
  // Anchored at the page's corner with a radius of one chunk: the four
  // chunks touching the corner's square of reach inside the page.
  frame(0, 0, 0);
  const f = CORE_BUILD.frame;
  f[FRAME.has_anchor] = 1;
  f[FRAME.anchor_x] = page_x + 128;
  f[FRAME.anchor_y] = page_y + 128;
  f[FRAME.render_dist] = 1;
  f[FRAME.range_hysteresis] = 64;
  CORE_BUILD.tick();
  const bases = [...mounted.values()].map(([x, y]) => `${x},${y}`).sort();
  assertEquals(bases, ["128,128", "128,64", "64,128", "64,64"]);

  // A chunk's step away: within the hysteresis, nothing leaves.
  f[FRAME.anchor_x] = page_x + 192;
  CORE_BUILD.tick();
  assertEquals(unmounted.length, 0);
  assert(mounted.size > 4, "the new neighbours come in");

  // The tile leaves: every chunk of the page goes.
  CORE_BUILD.page(PAGE, false);
  CORE_BUILD.tick();
  assertEquals(mounted.size, 0);
  CORE_BUILD.mounts = null;
});

Deno.test("core sched - the chunks the module mounts are module/world/frame/range.ts's in_range_chunks, anchor for anchor", () => {
  let mounted = new Set<string>();
  const ids = new Map<u32, string>();
  CORE_BUILD.mounts = {
    on_mount: (id, _page, bx, by) => {
      ids.set(id, `${bx},${by}`);
      mounted.add(`${bx},${by}`);
      CORE_BUILD.attach(id, {
        on_tier: (_g, _s, m) => release_chunk_mesh(m),
        on_runs: () => {},
        on_visible: () => {},
        on_layout: () => {},
        on_blend: () => {},
        on_evict: () => {},
        on_decor: (_s, d) => decor_block_release(d),
      });
    },
    on_unmount: (id) => void mounted.delete(ids.get(id)!),
  };
  const PAGE = ivec.gather(5n, 2n, 0n);
  const base = FROM_L3_IDX(PAGE);
  let seed = 17;
  const next = () => (seed = (seed * 1103515245 + 12345) % 2147483648);
  for (let k = 0; k < 20; k++) {
    // A fresh range each time: the page out, a tick to unmount, the page in.
    CORE_BUILD.page(PAGE, false);
    CORE_BUILD.tick();
    mounted = new Set();
    const ax = 5 * 1024 - 600 + (next() % 2200), ay = 2 * 1024 - 600 + (next() % 2200);
    const anchor = ivec.gather(BigInt(ax & ~63), BigInt(ay & ~63), 0n);
    const radius = 100 + (next() % 900);
    frame(0, 0, 0);
    const f = CORE_BUILD.frame;
    f[FRAME.has_anchor] = 1;
    f[FRAME.anchor_x] = ax & ~63;
    f[FRAME.anchor_y] = ay & ~63;
    f[FRAME.render_dist] = radius;
    f[FRAME.range_hysteresis] = 0;
    CORE_BUILD.page(PAGE, true);
    // A page mounts over a few frames, 64 chunks a frame.
    for (let k = 0; k < 8; k++) CORE_BUILD.tick();
    const expected = in_range_chunks(base, anchor, radius).map((o) => o.key).sort();
    assertEquals([...mounted].sort(), expected, `anchor ${ax & ~63},${ay & ~63} radius ${radius}`);
  }
  CORE_BUILD.page(PAGE, false);
  CORE_BUILD.tick();
  CORE_BUILD.mounts = null;
});

Deno.test("core sched - after a mount, or the view moving, the module asks for frames until a full recheck has run, and rests once nothing is pending", () => {
  CORE_BUILD.mounts = {
    on_mount: (id) =>
      CORE_BUILD.attach(id, {
        on_tier: (_g, _s, m) => release_chunk_mesh(m),
        on_runs: () => {},
        on_visible: () => {},
        on_layout: () => {},
        on_blend: () => {},
        on_evict: () => {},
        on_decor: (_s, d) => decor_block_release(d),
      }),
    on_unmount: () => {},
  };
  const PAGE = ivec.gather(7n, 7n, 0n);
  CORE_BUILD.page(PAGE, true);
  // A camera far above: every chunk mounts at the coarsest tier and lands quickly.
  frame(7 * 1024 + 512, 7 * 1024 + 512, 20000);
  const f = CORE_BUILD.frame;
  f[FRAME.has_anchor] = 1;
  f[FRAME.anchor_x] = 7 * 1024 + 512;
  f[FRAME.anchor_y] = 7 * 1024 + 512;
  f[FRAME.render_dist] = 64;
  f[FRAME.range_hysteresis] = 0;
  f[FRAME.delta_s] = 0.01;
  // The mount tick: nothing pending yet, and still a frame is asked for.
  assert(CORE_BUILD.tick(), "the mount asks for a frame");
  // Frames keep coming until the layouts land; then, the view still, none.
  let asked = true;
  for (let i = 0; i < 400 && asked; i++) asked = CORE_BUILD.tick();
  assert(!asked, "the world rests");
  assertEquals(CORE_BUILD.census().pending, 0);
  // The camera moves: a frame is asked for even before the next full recheck.
  f[FRAME.camera] += 1;
  assert(CORE_BUILD.tick(), "a moved view asks for a frame");
  CORE_BUILD.page(PAGE, false);
  CORE_BUILD.tick();
  CORE_BUILD.mounts = null;
});
