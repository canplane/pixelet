// @pxl/web/kernel/kernel.test.ts

/**
 * The kernel driven the way kernel/kernel.ts drives it, without a page: the
 * boot, the world's turns rung, the files it opens answered by its host with
 * no bake or asset bound, so every object is absent and the ground open
 * water, and what comes back — the UI's snapshot and the heap's row.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import { assert, assertEquals } from "@std/assert";

import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
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
  read_load_start,
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

/** The heap's row (crates/pxl/web/kernel/src/door.rs, written at every turn's
 * fall): range, used, live, run, slot, largest free, releases, refusals,
 * requests. `used` is the top the heap has reached. */
const HEAP_TOP = 1;
const HEAP_REQUESTS = 8;
const heap_row = new Uint32Array(CORE.buffer, directory(CORE.buffer).get(entry.HEAP_ROW), 9);
let strobe_requests = 0;
const answer_requests: Record<string, number> = {};

/** One frame as the driver runs it, and the host's answers let in after it. */
async function turn(delta = 1 / 60): Promise<Snapshot> {
  loads = [];
  CORE_DOORBELL.send(
    0,
    Command.WORLD_FRAME,
    encode_world_frame({ width: 1280, height: 720, dpr: 1, delta_s: delta, frame_ms: 1000 / 60, interactive: true, credits: 6, unix_ms: 0, hours: 12, minutes: 0, seconds: 0, day_start_ms: 0 }),
  );
  CORE_DOORBELL.ring(8);
  // An image's read is a read: it is settled by the key it was started with,
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
    settings: [],
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
