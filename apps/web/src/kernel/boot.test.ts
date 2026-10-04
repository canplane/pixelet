// @pxl/web/kernel/boot.test.ts

/**
 * The kernel driven as boot.ts drives it, without a page and with no bake
 * bound, so every object is absent and the ground is open water.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import { assert, assertEquals } from "@std/assert";

import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import {
  Command,
  encode_gpu_ready,
  encode_load_failed,
  encode_setting_set,
  encode_world_boot,
  encode_world_frame,
  encode_world_source,
  read_load_start,
  read_snapshot,
  read_world_snapshot,
  type Snapshot,
} from "@/kernel/abi/cmd.spec.ts";
import { PUBLISHED_SOURCE } from "@/config/published.spec.ts";
import { SETTINGS } from "@/feature/setting/schema.spec.ts";

import { CORE, CORE_DOORBELL, load_core } from "@/kernel/mem.ts";
import { heap_row, key as entry } from "@/kernel/abi/directory.spec.ts";
import { bytes_at, directory } from "@/kernel/module.ts";
import { start_sched } from "@/kernel/sched.ts";
import { acquireDevice } from "@/platform/web/webgpu.ts";

// The module the tests drive, loaded before any of them reads it, and its
// scheduler, never started: the turns are rung here.
await load_core();
start_sched();

let snapshot: Snapshot | null = null;
let loads: bigint[] = [];

CORE_DOORBELL.signal = (_tag, cmd, f) => {
  if (cmd === Command.WORLD_SNAPSHOT) {
    const { at, len } = read_world_snapshot(f!);
    snapshot = read_snapshot(new TreeReader(bytes_at(CORE.buffer, at, len)).read_branch());
  } else if (cmd === Command.LOAD_START) loads.push(read_load_start(f!).key);
};

const heap = new Uint32Array(CORE.buffer, directory(CORE.buffer).get(entry.HEAP_ROW), heap_row.WORDS);

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
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert(snapshot !== null, "the kernel posted its snapshot");
  return snapshot;
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
    settings: "",
  }),
  (r) => r,
);
CORE_DOORBELL.send(0, Command.WORLD_SOURCE, encode_world_source({ top_level: PUBLISHED_SOURCE.top_level }));

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

Deno.test("kernel - the settings commands reach the snapshot", () => {
  const tilt = SETTINGS.tilt_lock.key;
  const before = (await turn()).settings[tilt];
  command(Command.SETTING_SET, encode_setting_set({ key: tilt, value: 1 - before }));
  assertEquals((await turn()).settings[tilt], 1 - before);
  command(Command.SETTING_SET, encode_setting_set({ key: tilt, value: before }));
  assertEquals((await turn()).settings[tilt], before);
});

Deno.test("kernel - a settled turn asks the module heap for nothing", async () => {
  for (let i = 0; i < 200; i++) await turn();
  const [top, requests] = [heap[heap_row.USED], heap[heap_row.REQUESTS]];
  for (let i = 0; i < 100; i++) await turn();
  assertEquals([heap[heap_row.REQUESTS] - requests, heap[heap_row.USED] - top], [0, 0], "a hundred settled turns: no request, and the heap's top where it was");
});
