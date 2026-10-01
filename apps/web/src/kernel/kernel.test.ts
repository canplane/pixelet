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
import { Command as XputeCommand, encode_fetch_done, FetchOutcome, read_fetch_start } from "@xpute/runtime/abi/cmd.spec.ts";
import {
  Command,
  encode_gpu_ready,
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
const heap_row = new Uint32Array(CORE.buffer, directory(CORE.buffer).get(entry.HEAP_ROW), 9);

/** A response as the driver reports one: the body held for the kernel. */
function answer(f: Fetched, outcome: FetchOutcome, body: U8Array | null): void {
  const bytes = body?.byteLength ?? 0;
  const held = body !== null && bytes > 0 ? BODIES.hold(body) : 0;
  CORE_DOORBELL.send(0, XputeCommand.FETCH_DONE, encode_fetch_done({ key: f.key, outcome, body: held, body_bytes: bytes, fail: FetchFail.None, detail: 0, net_ms: 0 }));
  CORE_DOORBELL.ring(0);
}

function fail(f: Fetched): void {
  CORE_DOORBELL.send(0, XputeCommand.FETCH_DONE, encode_fetch_done({ key: f.key, outcome: FetchOutcome.Error, body: 0, body_bytes: 0, fail: FetchFail.Network, detail: 0, net_ms: 0 }));
  CORE_DOORBELL.ring(0);
}

/** One frame as the driver runs it, with every fetch answered at once. */
function turn(delta = 1 / 60): Turn {
  fetched = [];
  loads = [];
  CORE_DOORBELL.send(
    0,
    Command.WORLD_FRAME,
    encode_world_frame({ width: 1280, height: 720, dpr: 1, delta_s: delta, frame_ms: 1000 / 60, interactive: true, credits: 6, unix_ms: 0, hours: 12, minutes: 0, seconds: 0, day_start_ms: 0 }),
  );
  CORE_DOORBELL.ring(8);
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

/** One of the UI's commands, rung at once as the shell rings it. */
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
// What the shell hands over for a published source, read from the bundle.
CORE_DOORBELL.send(0, Command.WORLD_SOURCE, encode_world_source({ top_level: PUBLISHED_SOURCE.top_level }));

/** A device that takes every call and holds nothing: whatever is asked of
 * it, or of what it hands back, is another of the same. A chunk is drawn from
 * meshes on the device, so without one the world lists nothing to draw; the
 * shell reaches it through acquireDevice, and so does this. */
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
const device = await acquireDevice(() => {});
assert(device, "the stand-in device answered");
CORE_DOORBELL.send(0, Command.GPU_READY, encode_gpu_ready({ min_uniform_offset_alignment: device.align, timestamps: device.timestamps }));
CORE_DOORBELL.ring(0);
turn();

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
