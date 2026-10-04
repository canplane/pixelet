// @pxl/web/kernel/kernel.test.ts

/**
 * The kernel driven as kernel.ts drives it, without a page and with no bake
 * bound, so every object is absent and the ground is open water.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/abi/word.ts";
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import { FetchFail } from "@xpute/runtime/abi/fetch.spec.ts";
import { Command as XputeCommand, encode_fetch_done, FetchOutcome, read_fetch_start } from "@xpute/runtime/abi/cmd.spec.ts";
import {
  Command,
  encode_gpu_ready,
  encode_input_go_to,
  encode_input_view,
  encode_load_failed,
  encode_setting_set,
  encode_setting_toggle,
  encode_world_boot,
  encode_world_frame,
  encode_world_source,
  MapStatus,
  read_load_start,
  read_snapshot,
  read_world_frame,
  read_world_snapshot,
  type Snapshot,
} from "@/kernel/abi/cmd.spec.ts";
import { cube_types } from "@/catalog/cell/cube_type.spec.ts";
import { BAKE_KINDS, PUBLISHED_SOURCE } from "@/config/published.spec.ts";
import { SETTINGS } from "@/feature/setting/schema.spec.ts";

import { ELE_PADDED_SPAN, encode_terrain, pack_ground_cell, PAGE_CELLS } from "@/kernel/fixture.ts";

import { CORE, CORE_DOORBELL, load_core } from "@/kernel/mem.ts";
import { key as entry } from "@/kernel/abi/directory.spec.ts";
import { BODIES, bytes_at, directory } from "@/kernel/module.ts";
import { acquireDevice } from "@/platform/web/webgpu.ts";

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

CORE_DOORBELL.signal = (_tag, cmd, f) => {
  if (cmd === Command.WORLD_SNAPSHOT) {
    const { at, len } = read_world_snapshot(f!);
    snapshot = read_snapshot(new TreeReader(bytes_at(CORE.buffer, at, len)).read_branch());
  } else if (cmd === Command.WORLD_FRAME) draws = read_world_frame(f!).draws;
  else if (cmd === XputeCommand.FETCH_START) {
    const { key, source: bake, name } = read_fetch_start(f!);
    fetched.push({ key, bake, name, category: category_of(bake, name) });
  } else if (cmd === Command.LOAD_START) loads.push(read_load_start(f!).key);
};

/** Index of `used` in the heap's row. */
const HEAP_TOP = 1;
const HEAP_REQUESTS = 8;
const heap_row = new Uint32Array(CORE.buffer, directory(CORE.buffer).get(entry.HEAP_ROW), 9);
let strobe_requests = 0;
const answer_requests: Record<string, number> = {};

async function turn(delta = 1 / 60): Promise<Snapshot> {
  CORE_DOORBELL.send(
    0,
    Command.WORLD_FRAME,
    encode_world_frame({ width: 1280, height: 720, dpr: 1, delta_s: delta, frame_ms: 1000 / 60, interactive: true, credits: 6, unix_ms: 0, hours: 12, minutes: 0, seconds: 0, day_start_ms: 0 }),
  );
  CORE_DOORBELL.ring(8);
  strobe_requests += heap_row[HEAP_REQUESTS] - requests;
  const started = fetched;
  for (const f of started) {
    if (f.category === "terrain") answer(f, FetchOutcome.Read, terrain_body(Number(BigInt(f.name.slice(f.name.indexOf("/") + 1)) & 0xffffn)));
    else fail(f);
  }
  // An asset's read is a read: it is settled by the key it was started with,
  // and the kernel asks again in its own time.
  for (const key of loads) CORE_DOORBELL.send(0, Command.LOAD_FAILED, encode_load_failed({ key }));
  assert(snapshot !== null, "the kernel posted its snapshot");
  return { snapshot, fetched: started, draws };
}

function command(cmd: Command, pkt?: U8Array): void {
  CORE_DOORBELL.send(0, cmd, pkt);
  CORE_DOORBELL.ring(0);
}

CORE_DOORBELL.call(
  Command.WORLD_BOOT,
  encode_world_boot({
    development: false,
    handheld: false,
    geolocation: false,
    orientation: false,
    orientation_permission: false,
    orientation_silence_is_loss: true,
    settings: [],
    objects: 0,
    objects_bytes: 0,
  }),
  (r) => r,
);
CORE_DOORBELL.send(0, Command.WORLD_SOURCE, encode_world_source({ top_level: PUBLISHED_SOURCE.top_level }));
console.log(`heap: boot and world create ${heap_row[HEAP_REQUESTS]} requests, top ${heap_row[HEAP_TOP]}`);

/** A device that accepts every call: without one the world lists nothing to draw. */
function inert(): unknown {
  return new Proxy(() => {}, {
    get: (_, key) => {
      if (key === "then") return undefined;
      if (key === "lost") return new Promise(() => {});
      // `then` is undefined so awaiting a proxy does not hang; the error scope is the one awaited result.
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
const device = await acquireDevice(() => {});
assert(device, "the stand-in device answered");
CORE_DOORBELL.send(0, Command.GPU_READY, encode_gpu_ready({ min_uniform_offset_alignment: device.align, timestamps: device.timestamps }));
CORE_DOORBELL.ring(0);
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
  const tilt = SETTINGS.tilt_lock.key;
  const before = turn().snapshot.settings[tilt];
  command(Command.SETTING_TOGGLE, encode_setting_toggle({ key: tilt }));
  assertEquals(turn().snapshot.settings[tilt], 1 - before);
  command(Command.SETTING_SET, encode_setting_set({ key: tilt, value: before }));
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
