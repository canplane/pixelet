// @pxl/web/render/layer/pipeline/core_sched.test.ts

/**
 * The module's scheduler through the glue's channel: a chunk added with a
 * page and a frame written, ticks until its layout lands — every tier of it
 * handed over first — the blend and visibility it is told, a camera far
 * away releasing it, and a retain budget of nothing giving its tiers back.
 */

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { LodStep } from "@/render/lod.ts";
import { page } from "../terrain/page_fixture.ts";
import "../terrain/adapter.ts";
import "../object/adapter.ts";
import type { BakedChunkMesh } from "./chunk_mesher.ts";
import { CORE_BUILD, FRAME, release_chunk_mesh } from "./core_build.ts";

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
