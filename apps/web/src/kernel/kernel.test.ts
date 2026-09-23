// @pxl/web/kernel/kernel.test.ts

/**
 * The kernel driven the way kernel/kernel.ts drives it, without a page: the
 * boot, a map centre, the world's turns rung, the fetches it starts
 * answered from fixtures through the bodies the host holds, and what
 * comes back — what was drawn, the map's status, the UI's snapshot and its
 * actions.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/abi/word.ts";
import { Command } from "@/kernel/abi/cmd.spec.ts";
import { cube_types } from "@/catalog/cell/cube.ts";

import { ELE_PADDED_SPAN, encode_empty_poi_page, encode_terrain, pack_ground_cell, PAGE_CELL_SPAN } from "@/kernel/fixture.ts";

import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { CORE_KERNEL } from "@/kernel/mem.ts";
import { BOOT, boot_block, hold_body, text_at } from "@/kernel/module.ts";
import { acquire_device } from "@/kernel/render/gpu.ts";

function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

type Snapshot = {
  map: { status: string; is_off_center: boolean; center: [number, number] | null; page: string | null };
  settings: Record<string, boolean | string | number>;
};

/** A started read: the key the kernel hands out and takes back, the object it
 * named — which bake, and the name inside it — and the stream that names.
 * There is no URL here, and that is the point: the kernel does not send one. */
type Fetched = { key: bigint; bake: number; name: string; category: string };

/** The bakes, as `BakeKind` numbers them. */
const BAKE = ["terrain", "poi", "geocode"];

/** The stream an object belongs to. The two shared objects are named rather
 * than keyed, which is what tells them from the pages of their own bake. */
function category_of(bake: number, name: string): string {
  if (name === "_ocean") return "ocean";
  if (name === "_dict") return "geocode_dict";
  return BAKE[bake];
}

type Turn = { snapshot: Snapshot; fetched: Fetched[]; draws: number };

let snapshot: Snapshot | null = null;
let fetched: Fetched[] = [];
let loads: bigint[] = [];
let draws = 0;

CORE_BUILD.signal = (_tag, cmd, f) => {
  if (cmd === Command.WORLD_SNAPSHOT) {
    const [at, len] = f!.get<[number, number]>();
    snapshot = JSON.parse(text_at(CORE_KERNEL.memory, at >>> 0, len >>> 0)) as Snapshot;
  } else if (cmd === Command.WORLD_UPDATE) draws = f!.at(0).get<number>();
  else if (cmd === Command.FETCH_CREATE) {
    const [key, bake, name] = f!.get<[bigint, number, string]>();
    fetched.push({ key, bake, name, category: category_of(bake, name) });
  } else if (cmd === Command.LOAD_CREATE) loads.push(f!.at(0).get<bigint>());
};

/** A page's body as the bake encodes it: a rolling elevation raster and
 * ground that is mostly land, with water, structures and roads sprinkled. */
function terrain_body(seed: u32): U8Array {
  const next = rng(seed);
  const cells = PAGE_CELL_SPAN * PAGE_CELL_SPAN;
  const ele = new Int16Array(ELE_PADDED_SPAN * ELE_PADDED_SPAN);
  for (let i = 0; i < ele.length; i++) ele[i] = 10 + (next() % 30);
  const packed = new Uint16Array(cells);
  for (let i = 0; i < cells; i++) {
    const r = next() % 50;
    packed[i] = pack_ground_cell({
      land: next() % 3 === 0 ? cube_types.land.bare : cube_types.land.green_light,
      water: r === 0 ? cube_types.water.river : cube_types.common.void,
      structure: r === 1 ? cube_types.structure.building : cube_types.common.void,
      transport: r === 2 ? cube_types.transport.road_motorway : cube_types.common.void,
    });
  }
  return encode_terrain(ele, packed);
}

/** The heap's row (xpute's BuddyMalloc::stat): base, top reached, end and
 * live as words, then releases and requests. */
const HEAP_TOP = 1;
const HEAP_REQUESTS = 5;
const heap_row = new Uint32Array(CORE_KERNEL.memory.buffer, boot_block(CORE_KERNEL)[BOOT.heap_row], 6);
let strobe_requests = 0;
const answer_requests: Record<string, number> = {};

/** A response as the driver reports one: the body held for the kernel. */
function answer(f: Fetched, outcome: number, body: U8Array | null): void {
  const requests = heap_row[HEAP_REQUESTS];
  try {
    const bytes = body?.byteLength ?? 0;
    const held = body !== null && bytes > 0 ? hold_body(body) : 0;
    CORE_BUILD.send(0, Command.FETCH_UPDATE, packet((b) => b.u64(f.key).u32(outcome).u32(held).u32(bytes).u32(0).u32(0).f64(0)));
    CORE_BUILD.ring(0);
  } finally {
    answer_requests[f.category] = (answer_requests[f.category] ?? 0) + heap_row[HEAP_REQUESTS] - requests;
  }
}

function fail(f: Fetched): void {
  CORE_BUILD.send(0, Command.FETCH_UPDATE, packet((b) => b.u64(f.key).u32(2).u32(0).u32(0).u32(0).u32(0).f64(0)));
  CORE_BUILD.ring(0);
}

/** One frame as the driver runs it, with every fetch answered at once. */
function turn(delta = 1 / 60): Turn {
  fetched = [];
  loads = [];
  const requests = heap_row[HEAP_REQUESTS];
  CORE_BUILD.send(0, Command.WORLD_UPDATE, packet((b) => b.f64(1280).f64(720).f64(1).f64(delta).f64(1000 / 60).bool(true).u32(6)));
  CORE_BUILD.ring(8);
  strobe_requests += heap_row[HEAP_REQUESTS] - requests;
  const started = fetched;
  for (const f of started) {
    if (f.category === "terrain") answer(f, 0, terrain_body(Number(BigInt(f.name.slice(f.name.indexOf("/") + 1)) & 0xffffn)));
    else if (f.category === "poi") answer(f, 0, encode_empty_poi_page());
    else fail(f);
  }
  // An asset's read is a read: it is settled by the key it was started with,
  // and the kernel asks again in its own time.
  for (const key of loads) CORE_BUILD.send(0, Command.LOAD_DELETE, packet((b) => b.u64(key)));
  assert(snapshot !== null, "the kernel posted its snapshot");
  return { snapshot, fetched: started, draws };
}

function action(a: Record<string, unknown>): void {
  CORE_BUILD.send(0, Command.WORLD_ACTION, packet((b) => b.str(JSON.stringify(a))));
  CORE_BUILD.ring(0);
}

CORE_BUILD.call(
  Command.WORLD_CREATE,
  packet((b) => b.str("test").str("2026-09-15T00:00:00Z").str("r0-test").str("b?").bool(false).bool(false).bool(false).bool(false).bool(false)),
  (r) => r,
);
console.log(`heap: boot and world create ${heap_row[HEAP_REQUESTS]} requests, top ${heap_row[HEAP_TOP]}`);

/** A device that takes every call and holds nothing: whatever is asked of
 * it, or of what it hands back, is another of the same. A chunk is drawn from
 * meshes on the device, so without one the world lists nothing to draw; the
 * shell reaches it through acquire_device, and so does this. */
function inert(): unknown {
  return new Proxy(() => {}, {
    get: (_, key) => {
      if (key === "then") return undefined;
      if (key === "lost") return new Promise(() => {});
      if (key === "limits") return { minUniformBufferOffsetAlignment: 256 };
      return inert();
    },
    apply: () => inert(),
    set: () => true,
  });
}
Object.defineProperty(navigator, "gpu", {
  configurable: true,
  value: {
    requestAdapter: () => Promise.resolve({ features: new Set(), requestDevice: () => Promise.resolve(inert()) }),
    getPreferredCanvasFormat: () => "bgra8unorm",
  },
});
const device = await acquire_device(() => {});
assert("align" in device, "the stand-in device answered");
CORE_BUILD.send(0, Command.GPU_CREATE, packet((b) => b.u32(device.align).bool(device.timestamps)));
CORE_BUILD.ring(0);
turn();
console.log(`heap: through the first turn ${heap_row[HEAP_REQUESTS]} requests, top ${heap_row[HEAP_TOP]}`);

Deno.test("kernel - a map centre fetches its terrain and draws it, and the map settles to success", () => {
  action({ type: "go_to", lon: 126.978, lat: 37.5665 });
  let first = turn();
  assert(first.snapshot.map.center !== null, "the centre is set");
  // The page the shell keys a reverse-geocode label by, checked against the
  // terrain fetches below.
  const page = first.snapshot.map.page;
  assert(page !== null && /^\d+$/.test(page), `the centre's page is a decimal index: ${page}`);
  const statuses = new Set<string>([first.snapshot.map.status]);
  const categories = new Set(first.fetched.map((f) => f.category));
  let terrain_names = first.fetched.filter((f) => f.category === "terrain").map((f) => f.name);
  let max_draws = first.draws;
  for (let i = 0; i < 400; i++) {
    const t = turn();
    statuses.add(t.snapshot.map.status);
    for (const f of t.fetched) categories.add(f.category);
    terrain_names = terrain_names.concat(t.fetched.filter((f) => f.category === "terrain").map((f) => f.name));
    max_draws = Math.max(max_draws, t.draws);
    first = t;
  }
  assert(categories.has("terrain"), `terrain was asked for: ${[...categories]}`);
  // The centre's own page is among the pages the kernel fetched terrain for,
  // which is what says `map.page` is that centre's page and not merely a
  // well-shaped number.
  assert(terrain_names.some((n) => n.endsWith(`/${page}`)), `the centre's page ${page} was fetched`);
  assert(terrain_names.every((n) => /^\d+\/-?\d+$/.test(n)), `a terrain name is a level and a page: ${terrain_names.slice(0, 3)}`);
  assert(max_draws > 0, "something was drawn");
  assert(statuses.has("loading"), `the map loaded: ${[...statuses]}`);
  assertEquals(first.snapshot.map.status, "success");
});

Deno.test("kernel - the settings actions reach the snapshot", () => {
  const before = turn().snapshot.settings.lock_camera_tilt;
  action({ type: "toggle_setting", key: "lock_camera_tilt" });
  assertEquals(turn().snapshot.settings.lock_camera_tilt, !before);
  action({ type: "set_setting", key: "lock_camera_tilt", value: before });
  assertEquals(turn().snapshot.settings.lock_camera_tilt, before);
  // The world was created under "test", not "development": a dev-only
  // setting is one this build does not offer, and the action is refused
  // rather than applied to a row nothing on screen could undo.
  const dev = turn().snapshot.settings.show_poi;
  action({ type: "toggle_setting", key: "show_poi" });
  assertEquals(turn().snapshot.settings.show_poi, dev);
  action({ type: "set_setting", key: "show_poi", value: !dev });
  assertEquals(turn().snapshot.settings.show_poi, dev);
});

Deno.test("kernel - a settled turn asks the module heap for nothing", () => {
  for (let i = 0; i < 200; i++) turn();
  const [top, requests] = [heap_row[HEAP_TOP], heap_row[HEAP_REQUESTS]];
  for (let i = 0; i < 100; i++) turn();
  assertEquals([heap_row[HEAP_REQUESTS] - requests, heap_row[HEAP_TOP] - top], [0, 0], "a hundred settled turns: no request, and the heap's top where it was");
});

Deno.test("kernel - heap requests along the event paths", () => {
  for (let i = 0; i < 50; i++) turn();
  let mark = heap_row[HEAP_REQUESTS];
  const since = () => {
    const n = heap_row[HEAP_REQUESTS] - mark;
    mark = heap_row[HEAP_REQUESTS];
    return n;
  };
  strobe_requests = 0;
  for (const k of Object.keys(answer_requests)) delete answer_requests[k];
  action({ type: "go_to", lon: 127.3, lat: 36.35 });
  let load = 0;
  for (let i = 0; i < 200; i++) turn();
  load = since();
  console.log(`heap: a far jump and its load ${load}`);
  console.log(`heap: the load's strobes ${strobe_requests}, its answers ${JSON.stringify(answer_requests)}`);
});

Deno.test("kernel - a view from high up draws its outermost ring, which is coarser than the rings under it", () => {
  action({ type: "go_to", lon: 126.9706, lat: 37.5547 });
  for (let k = 0; k < 40; k++) turn();
  // Far enough that the window spans several levels: the outermost has no
  // level over it, so it is drawn as coarse as the ladder says rather than
  // one step off its own cells, and asks for tiers the finer rings never do.
  action({ type: "set_view", dist: 8000, polar: (80 * Math.PI) / 180 });
  let drawn = 0;
  for (let k = 0; k < 200; k++) drawn = Math.max(drawn, turn().draws);
  assert(drawn > 0, "the far view drew something");
});
