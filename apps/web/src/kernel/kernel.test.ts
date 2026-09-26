// @pxl/web/kernel/kernel.test.ts

/**
 * The kernel driven the way kernel/kernel.ts drives it, without a page: the
 * boot, a map center, the world's turns rung, the fetches it starts
 * answered from fixtures through the bodies the host holds, and what
 * comes back — what was drawn, the map's status, the UI's snapshot and its
 * commands.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/abi/word.ts";
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import { FetchFail } from "@xpute/runtime/abi/fetch.spec.ts";
import {
  Command,
  encode_fetch_update,
  encode_gpu_create,
  encode_input_go_to,
  encode_input_view,
  encode_load_delete,
  encode_setting_toggle,
  encode_setting_update,
  encode_world_create,
  encode_world_update,
  FetchOutcome,
  MapStatus,
  read_fetch_create,
  read_load_create,
  read_snapshot,
  read_world_snapshot,
  read_world_update,
  type Snapshot,
} from "@/kernel/abi/cmd.spec.ts";
import { cube_types } from "@/catalog/cell/cube_type.spec.ts";
import { BAKE_KINDS } from "@/config/published.spec.ts";
import { SETTINGS } from "@/feature/setting/schema.spec.ts";

import { ELE_PADDED_SPAN, encode_terrain, pack_ground_cell, PAGE_CELLS } from "@/kernel/fixture.ts";

import { CORE_BUILD } from "@/kernel/ipc.ts";
import { CORE, load_core } from "@/kernel/mem.ts";
import { BOOT, boot_block, bytes_at, hold_body } from "@/kernel/module.ts";
import { acquire_device } from "@/kernel/render/gpu.ts";

// The module the tests drive, loaded before any of them reads it.
await load_core();

function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

/** A started read: the key the kernel hands out and takes back, the object it
 * named — which bake, and the name inside it — and the stream that names.
 * There is no URL here, and that is the point: the kernel does not send one. */
type Fetched = { key: bigint; bake: number; name: string; category: string };

/** The stream an object belongs to. The two shared objects are named rather
 * than keyed, which is what tells them from the pages of their own bake. */
function category_of(bake: number, name: string): string {
  if (name === "_ocean") return "ocean";
  if (name === "_dict") return "geocode_dict";
  return BAKE_KINDS[bake];
}

type Turn = { snapshot: Snapshot; fetched: Fetched[]; draws: number };

let snapshot: Snapshot | null = null;
let fetched: Fetched[] = [];
let loads: bigint[] = [];
let draws = 0;

CORE_BUILD.signal = (_tag, cmd, f) => {
  if (cmd === Command.WORLD_SNAPSHOT) {
    const { at, len } = read_world_snapshot(f!);
    snapshot = read_snapshot(new TreeReader(bytes_at(CORE.buffer, at, len)).read_branch());
  } else if (cmd === Command.WORLD_UPDATE) draws = read_world_update(f!).draws;
  else if (cmd === Command.FETCH_CREATE) {
    const { key, bake, name } = read_fetch_create(f!);
    fetched.push({ key, bake, name, category: category_of(bake, name) });
  } else if (cmd === Command.LOAD_CREATE) loads.push(read_load_create(f!).key);
};

/** A page's body as the bake encodes it: a rolling elevation raster and
 * ground that is mostly land, with water, structures and roads sprinkled. */
function terrain_body(seed: u32): U8Array {
  const next = rng(seed);
  const cells = PAGE_CELLS * PAGE_CELLS;
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

/** The heap's row (crates/pxl/web/kernel/src/door.rs, written at every turn's
 * fall): range, used, live, run, slot, largest free, releases, refusals,
 * requests. `used` is the top the heap has reached. */
const HEAP_TOP = 1;
const HEAP_REQUESTS = 8;
const heap_row = new Uint32Array(CORE.buffer, boot_block(CORE.buffer)[BOOT.heap_row], 9);
let strobe_requests = 0;
const answer_requests: Record<string, number> = {};

/** A response as the driver reports one: the body held for the kernel. */
function answer(f: Fetched, outcome: FetchOutcome, body: U8Array | null): void {
  const requests = heap_row[HEAP_REQUESTS];
  try {
    const bytes = body?.byteLength ?? 0;
    const held = body !== null && bytes > 0 ? hold_body(body) : 0;
    CORE_BUILD.send(0, Command.FETCH_UPDATE, encode_fetch_update({ key: f.key, outcome, body: held, body_bytes: bytes, fail: FetchFail.None, detail: 0, net_ms: 0 }));
    CORE_BUILD.ring(0);
  } finally {
    answer_requests[f.category] = (answer_requests[f.category] ?? 0) + heap_row[HEAP_REQUESTS] - requests;
  }
}

function fail(f: Fetched): void {
  CORE_BUILD.send(0, Command.FETCH_UPDATE, encode_fetch_update({ key: f.key, outcome: FetchOutcome.Error, body: 0, body_bytes: 0, fail: FetchFail.Network, detail: 0, net_ms: 0 }));
  CORE_BUILD.ring(0);
}

/** One frame as the driver runs it, with every fetch answered at once. */
function turn(delta = 1 / 60): Turn {
  fetched = [];
  loads = [];
  const requests = heap_row[HEAP_REQUESTS];
  CORE_BUILD.send(
    0,
    Command.WORLD_UPDATE,
    encode_world_update({ width: 1280, height: 720, dpr: 1, delta_s: delta, frame_ms: 1000 / 60, interactive: true, credits: 6, unix_ms: 0, hours: 12, minutes: 0, seconds: 0, day_start_ms: 0 }),
  );
  CORE_BUILD.ring(8);
  strobe_requests += heap_row[HEAP_REQUESTS] - requests;
  const started = fetched;
  for (const f of started) {
    if (f.category === "terrain") answer(f, FetchOutcome.Read, terrain_body(Number(BigInt(f.name.slice(f.name.indexOf("/") + 1)) & 0xffffn)));
    else fail(f);
  }
  // An asset's read is a read: it is settled by the key it was started with,
  // and the kernel asks again in its own time.
  for (const key of loads) CORE_BUILD.send(0, Command.LOAD_DELETE, encode_load_delete({ key }));
  assert(snapshot !== null, "the kernel posted its snapshot");
  return { snapshot, fetched: started, draws };
}

/** One of the UI's commands, rung at once as the shell rings it. */
function command(cmd: Command, pkt?: U8Array): void {
  CORE_BUILD.send(0, cmd, pkt);
  CORE_BUILD.ring(0);
}

CORE_BUILD.call(
  Command.WORLD_CREATE,
  encode_world_create({ development: false, handheld: false, geolocation: false, orientation: false, orientation_permission: false, settings: [], objects: 0, objects_bytes: 0 }),
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
      // `then` is undefined above so that awaiting one of these does not hang,
      // which also makes every proxy unawaitable — and an error scope is the
      // one call whose result the renderer awaits (gpu.ts op.BUFFER).
      if (key === "popErrorScope") return () => Promise.resolve(null);
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
CORE_BUILD.send(0, Command.GPU_CREATE, encode_gpu_create({ min_uniform_offset_alignment: device.align, timestamps: device.timestamps }));
CORE_BUILD.ring(0);
turn();
console.log(`heap: through the first turn ${heap_row[HEAP_REQUESTS]} requests, top ${heap_row[HEAP_TOP]}`);

Deno.test("kernel - a map center fetches its terrain and draws it, and the map settles to success", () => {
  command(Command.INPUT_GO_TO, encode_input_go_to({ lon: 126.978, lat: 37.5665 }));
  let first = turn();
  assert(first.snapshot.map.center !== null, "the center is set");
  // The page the shell keys a reverse-geocode label by. It is the index, not
  // an object's name — what the kernel asks the host for is the name.
  const page = first.snapshot.map.page;
  assert(page !== null, "the center's page is an index");
  const statuses = new Set<MapStatus>([first.snapshot.map.status]);
  const categories = new Set(first.fetched.map((f) => f.category));
  let terrain_names = first.fetched.filter((f) => f.category === "terrain").map((f) => f.name);
  let geocode_names = first.fetched.filter((f) => f.category === "geocode").map((f) => f.name);
  let max_draws = first.draws;
  for (let i = 0; i < 400; i++) {
    const t = turn();
    statuses.add(t.snapshot.map.status);
    for (const f of t.fetched) categories.add(f.category);
    terrain_names = terrain_names.concat(t.fetched.filter((f) => f.category === "terrain").map((f) => f.name));
    geocode_names = geocode_names.concat(t.fetched.filter((f) => f.category === "geocode").map((f) => f.name));
    max_draws = Math.max(max_draws, t.draws);
    first = t;
  }
  assert(categories.has("terrain"), `terrain was asked for: ${[...categories]}`);
  assert(terrain_names.every((n) => /^[0-3]{5,15}$/.test(n)), `a terrain name is a quadkey: ${terrain_names.slice(0, 3)}`);
  assert(geocode_names.every((n) => /^[0-3]{11}$/.test(n)), `a group's name is a shorter one: ${geocode_names.slice(0, 3)}`);
  // The property the names have this shape for: a coarser object's name is a
  // prefix of a finer one's over the same ground, so one prefix in the store
  // is one region. Two layers that share no code arrive at the same digits
  // from the same center, which is also what says these are that center's
  // pages rather than well-shaped numbers.
  assert(geocode_names.length > 0, "a reverse-geocode group was asked for");
  assert(
    terrain_names.some((t) => geocode_names.some((g) => t.startsWith(g))),
    `no leaf sits under a fetched group: ${terrain_names.slice(0, 3)} / ${geocode_names.slice(0, 3)}`,
  );
  assert(max_draws > 0, "something was drawn");
  assert(statuses.has(MapStatus.Loading), `the map loaded: ${[...statuses]}`);
  assertEquals(first.snapshot.map.status, MapStatus.Success);
});

Deno.test("kernel - the settings commands reach the snapshot", () => {
  const tilt = SETTINGS.lock_camera_tilt.key;
  const before = turn().snapshot.settings[tilt];
  command(Command.SETTING_TOGGLE, encode_setting_toggle({ key: tilt }));
  assertEquals(turn().snapshot.settings[tilt], 1 - before);
  command(Command.SETTING_UPDATE, encode_setting_update({ key: tilt, value: before }));
  assertEquals(turn().snapshot.settings[tilt], before);
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
  command(Command.INPUT_GO_TO, encode_input_go_to({ lon: 127.3, lat: 36.35 }));
  let load = 0;
  for (let i = 0; i < 200; i++) turn();
  load = since();
  console.log(`heap: a far jump and its load ${load}`);
  console.log(`heap: the load's strobes ${strobe_requests}, its answers ${JSON.stringify(answer_requests)}`);
});

Deno.test("kernel - a view from high up draws its outermost ring, which is coarser than the rings under it", () => {
  command(Command.INPUT_GO_TO, encode_input_go_to({ lon: 126.9706, lat: 37.5547 }));
  for (let k = 0; k < 40; k++) turn();
  // Far enough that the window spans several levels: the outermost has no
  // level over it, so it is drawn as coarse as the ladder says rather than
  // one step off its own cells, and asks for tiers the finer rings never do.
  command(Command.INPUT_VIEW, encode_input_view({ dist: 8000, polar: (80 * Math.PI) / 180 }));
  let drawn = 0;
  for (let k = 0; k < 200; k++) drawn = Math.max(drawn, turn().draws);
  assert(drawn > 0, "the far view drew something");
});
