// @pxl/web/kernel/kernel.test.ts

/**
 * The kernel driven as kernel.ts drives it, without a page and with no bake
 * bound, so every object is absent and the ground is open water.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import { assert, assertEquals } from "@std/assert";

import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import {
  Command,
  encode_gpu_ready,
  encode_setting_set,
  encode_setting_toggle,
  encode_world_boot,
  encode_world_frame,
  encode_world_source,
  read_snapshot,
  read_world_snapshot,
  type Snapshot,
} from "@/kernel/abi/cmd.spec.ts";
import { PUBLISHED_SOURCE } from "@/config/published.spec.ts";
import { SETTINGS } from "@/feature/setting/schema.spec.ts";

import { CORE, CORE_DOORBELL, load_core } from "@/kernel/mem.ts";
import { key as entry } from "@/kernel/abi/directory.spec.ts";
import { bytes_at, directory } from "@/kernel/module.ts";
import { start_sched } from "@/kernel/sched.ts";
import { acquireDevice } from "@/platform/web/webgpu.ts";

await load_core();
start_sched();

let snapshot: Snapshot | null = null;

CORE_DOORBELL.signal = (_tag, cmd, f) => {
  if (cmd === Command.WORLD_SNAPSHOT) {
    const { at, len } = read_world_snapshot(f!);
    snapshot = read_snapshot(new TreeReader(bytes_at(CORE.buffer, at, len)).read_branch());
  }
};

/** Index of `used` in the heap's row. */
const HEAP_TOP = 1;
const HEAP_REQUESTS = 8;
const heap_row = new Uint32Array(CORE.buffer, directory(CORE.buffer).get(entry.HEAP_ROW), 9);

async function turn(delta = 1 / 60): Promise<Snapshot> {
  CORE_DOORBELL.send(
    0,
    Command.WORLD_FRAME,
    encode_world_frame({ width: 1280, height: 720, dpr: 1, delta_s: delta, frame_ms: 1000 / 60, interactive: true, credits: 6, unix_ms: 0, hours: 12, minutes: 0, seconds: 0, day_start_ms: 0 }),
  );
  CORE_DOORBELL.ring(8);
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
await turn();

Deno.test("kernel - the settings commands reach the snapshot", async () => {
  const tilt = SETTINGS.tilt_lock.key;
  const before = (await turn()).settings[tilt];
  command(Command.SETTING_TOGGLE, encode_setting_toggle({ key: tilt }));
  assertEquals((await turn()).settings[tilt], 1 - before);
  command(Command.SETTING_SET, encode_setting_set({ key: tilt, value: before }));
  assertEquals((await turn()).settings[tilt], before);
});

Deno.test("kernel - a settled turn asks the module heap for nothing", async () => {
  for (let i = 0; i < 200; i++) await turn();
  const [top, requests] = [heap_row[HEAP_TOP], heap_row[HEAP_REQUESTS]];
  for (let i = 0; i < 100; i++) await turn();
  assertEquals([heap_row[HEAP_REQUESTS] - requests, heap_row[HEAP_TOP] - top], [0, 0], "a hundred settled turns: no request, and the heap's top where it was");
});
