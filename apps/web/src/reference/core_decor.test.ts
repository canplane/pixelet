// @pxl/web/reference/core_decor.test.ts

/**
 * The module's decor build against the TypeScript one it replaced
 * (decor/decor_kinds.ts's build_decor_instances), instance for instance:
 * the same scatter from the same streams, a placed tree with its variant
 * and heading, the eraser's hide as the flag, and a cube standing where a
 * scattered instance would have — on the tier every kind is drawn at, and
 * on a coarse one where only the placed tree is.
 */

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { ELE_PADDED_SPAN, PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { type Ground, type Terrain } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import type { LodStep } from "@/module/world/lod.ts";
import { DECOR_BUNDLES } from "@/module/world/decor/decor_registry.ts";
import { build_decor_instances } from "@/reference/decor_build.ts";
import { instances_release } from "@/reference/decor_instance.ts";
import { cell_index, cell_key, get_objects_in_cell, place, place_decor, remove, set_virtual_hide } from "@/module/world/object/store.ts";
import { useTileStore } from "@/module/world/terrain/store.ts";
import { into_core } from "@/module/world/terrain/resident.ts";
import { page } from "@/reference/page_fixture.ts";
import "@/reference/terrain_layer.ts";
import "@/reference/object_layer.ts";
import { CORE_BUILD, FRAME, release_chunk_mesh } from "@/wasm/core_build.ts";
import type { MeshBlock } from "@/render/mesh/mesh_block.ts";
import { decor_block_release, type DecorBlock, type DecorInstances, NO_ENTRY } from "@/render/mesh/decor_block.ts";

const PAGE_IDX = ivec.gather(0n, 0n, 0n);
const BASE_X = 320, BASE_Y = 448;
const terrain = page(41);

/** A page where every kind has ground — the fixture's random ground seldom
 * scatters a boat, a car or a train: grass everywhere, rivers across every
 * other band of rows, roads and rails down stripes of columns between
 * them, and one road across so the alignment's votes decide both ways. */
function staged(): Terrain {
  const ele = new Int16Array(ELE_PADDED_SPAN * ELE_PADDED_SPAN);
  for (let i = 0; i < ele.length; i++) ele[i] = 20 + ((i * 7919) % 13);
  const ground = {} as Ground;
  for (const key of ["land", "water", "structure", "transport"] as const) ground[key] = new Uint8Array(PAGE_CELL_SPAN * PAGE_CELL_SPAN);
  ground.land.fill(cube_types.land.GREEN_LIGHT);
  for (let y = 0; y < PAGE_CELL_SPAN; y++) {
    for (let x = 0; x < PAGE_CELL_SPAN; x++) {
      const i = y * PAGE_CELL_SPAN + x;
      if ((y >> 5) & 1) {
        ground.water[i] = cube_types.water.RIVER;
        continue;
      }
      if (y % 32 === 20) ground.transport[i] = cube_types.transport.ROAD_SECONDARY;
      else if (x % 16 < 6) ground.transport[i] = cube_types.transport.ROAD_PRIMARY;
      else if (x % 16 >= 8 && x % 16 < 12) ground.transport[i] = cube_types.transport.RAIL_MAIN;
    }
  }
  return into_core({ ele, ground }, false);
}

type Heard = { decor: Map<LodStep, DecorBlock>; layouts: { primary: LodStep; ready: boolean }[] };

function chunk(terrain: Terrain, base_x: number = BASE_X, base_y: number = BASE_Y): { id: u32; heard: Heard } {
  const heard: Heard = { decor: new Map(), layouts: [] };
  const sink = {
    on_tier: (_g: u32, _s: LodStep, mesh: MeshBlock) => release_chunk_mesh(mesh),
    on_runs: () => {},
    on_visible: () => {},
    on_layout: (primary: LodStep, _s: LodStep | null, ready: boolean) => {
      if (ready) assert(heard.decor.has(primary), `layout ${primary} before its decor`);
      heard.layouts.push({ primary, ready });
    },
    on_blend: () => {},
    on_evict: (step: LodStep) => {
      const d = heard.decor.get(step);
      if (d) decor_block_release(d);
      heard.decor.delete(step);
    },
    on_decor: (step: LodStep, d: DecorBlock) => {
      const old = heard.decor.get(step);
      if (old) decor_block_release(old);
      heard.decor.set(step, d);
    },
  };
  const id = CORE_BUILD.add(sink, ivec.gather(BigInt(base_x), BigInt(base_y), 0n), base_x, base_y, base_x, base_y, 7);
  CORE_BUILD.source(id, terrain, false);
  return { id, heard };
}

function frame(cx: number, cy: number, cz: number): void {
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
  f[FRAME.retain_budget_bytes] = 1 << 30;
  f[FRAME.show_decor] = 1;
}

/** Ticks until the layout lands; the step it shows. */
function land(heard: Heard): LodStep {
  heard.layouts.length = 0;
  const landed = () => heard.layouts.at(-1)?.ready === true;
  for (let i = 0; i < 50 && !landed(); i++) CORE_BUILD.tick();
  assert(landed(), "the layout never landed");
  return heard.layouts.at(-1)!.primary;
}

/** The TypeScript build's groups in the module's order: every bundle's
 * variants, bundle by bundle. */
function reference(terrain: Terrain, step: LodStep, base_x: number = BASE_X, base_y: number = BASE_Y): (DecorInstances | null)[] {
  useTileStore.getState().set_tile(PAGE_IDX, { terrain });
  return DECOR_BUNDLES.flatMap((bundle) => build_decor_instances({ terrain }, step, PAGE_IDX, base_x, base_y, bundle));
}

function assert_same(actual: DecorBlock, expected: (DecorInstances | null)[], at: string): void {
  assertEquals(actual.groups.length, expected.length, `${at}: groups`);
  expected.forEach((want, g) => {
    const got = actual.groups[g];
    assertEquals(got?.len ?? 0, want?.len ?? 0, `${at}: group ${g} count`);
    if (!got || !want) return;
    for (const lane of ["x", "y", "z", "rotation_z", "scale", "flags", "band", "pos"] as const) {
      assertEquals(Array.from(got[lane]), Array.from(want[lane]), `${at}: group ${g} ${lane}`);
    }
  });
}

function release_reference(groups: (DecorInstances | null)[]): void {
  for (const g of groups) if (g) instances_release(g);
}

function placed_id(x: bigint, y: bigint, z: bigint): string {
  const [ix, iy, iz] = cell_index({ i: ivec.gather(x, y, z), f: 0n });
  return get_objects_in_cell(cell_key(ix, iy, iz))[0].id;
}

Deno.test("core decor - the module scatters what build_decor_instances scattered, and stands the placed tree where it stood", () => {
  const tree_i = ivec.gather(BigInt(BASE_X + 12), BigInt(BASE_Y + 40), 30n);
  place_decor(tree_i, 0n, 1, "tree");
  const tree = placed_id(BigInt(BASE_X + 12), BigInt(BASE_Y + 40), 30n);

  const { id, heard } = chunk(terrain);
  frame(BASE_X + 32, BASE_Y + 32, 60);
  const fine = land(heard);
  assertEquals(fine, 1);
  const expected = reference(terrain, 1);
  assert(expected.some((g) => g !== null && g.band[0] === NO_ENTRY), "the fixture scatters something");
  assert(expected.some((g) => g !== null && Array.from(g.band).some((b) => b !== NO_ENTRY)), "the placed tree is in the reference");
  assert_same(heard.decor.get(1)!, expected, "fine, tree placed");
  release_reference(expected);

  // The eraser's hide is the flag on the same instance.
  set_virtual_hide(tree);
  CORE_BUILD.decor_now(id, true);
  const hidden = reference(terrain, 1);
  assert(hidden.some((g) => g !== null && Array.from(g.flags).some((f) => f !== 0)), "the reference hides the tree");
  assert_same(heard.decor.get(1)!, hidden, "fine, tree hidden");
  release_reference(hidden);
  set_virtual_hide(null);

  // A cube where a scattered instance stands takes its cell.
  const scattered = heard.decor.get(1)!.groups.find((g) => g !== null && g.band[0] === NO_ENTRY)!;
  const [sx, sy, sz] = [Math.floor(scattered.x[0]), Math.floor(scattered.y[0]), Math.floor(scattered.z[0])];
  place(ivec.gather(BigInt(BASE_X + sx), BigInt(BASE_Y + sy), BigInt(sz)), 0n, 1, "red");
  const cube = placed_id(BigInt(BASE_X + sx), BigInt(BASE_Y + sy), BigInt(sz));
  CORE_BUILD.source(id, terrain, false);
  CORE_BUILD.build_now(id);
  const occupied = reference(terrain, 1);
  assert_same(heard.decor.get(1)!, occupied, "fine, a cube on a scattered cell");
  release_reference(occupied);

  // Far away: a coarse tier draws no kind, and the placed tree stays.
  frame(BASE_X + 32, BASE_Y + 32, 2000);
  const coarse = land(heard);
  assert(coarse >= 8, `a coarse tier far away, not ${coarse}`);
  const far = reference(terrain, coarse);
  assert(far.every((g) => g === null || Array.from(g.band).every((b) => b !== NO_ENTRY)), "nothing scattered on a coarse tier");
  assert_same(heard.decor.get(coarse)!, far, `coarse ${coarse}`);
  release_reference(far);

  remove(cube);
  remove(tree);
  CORE_BUILD.remove(id);
  CORE_BUILD.pump();
  for (const d of heard.decor.values()) decor_block_release(d);
});

Deno.test("core decor - boats on the river, cars and trains along the road and the rail, turned as the TypeScript turned them", () => {
  const terrain = staged();
  const counts = DECOR_BUNDLES.map(() => 0);
  for (const [bx, by] of [[0, 0], [64, 0], [0, 64], [64, 64], [128, 128]]) {
    const { id, heard } = chunk(terrain, bx, by);
    frame(bx + 32, by + 32, 60);
    assertEquals(land(heard), 1);
    const expected = reference(terrain, 1, bx, by);
    let g = 0;
    DECOR_BUNDLES.forEach((bundle, k) => {
      for (let v = 0; v < bundle.seed.variant_count; v++) counts[k] += expected[g++]?.len ?? 0;
    });
    assert_same(heard.decor.get(1)!, expected, `staged ${bx},${by}`);
    release_reference(expected);
    CORE_BUILD.remove(id);
    CORE_BUILD.pump();
    for (const d of heard.decor.values()) decor_block_release(d);
  }
  assert(counts.every((n) => n > 0), `every kind scatters: ${counts}`);
});
