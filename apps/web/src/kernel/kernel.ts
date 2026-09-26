// @pxl/web/kernel/kernel.ts

/**
 * The completion ring's signals, and the boot that starts everything.
 *
 * A turn leaves on the completion ring what the kernel asks of the host — the
 * sensors' watches, the fetches to start and cancel, the loads, the UI's
 * snapshot when it changed — and this is where each is handed to whoever runs
 * it. Nothing is decided here: every branch is one line to the module that
 * does the work.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import type { BranchCursor } from "@xpute/core/wire/xtp/mod.ts";
import {
  Command,
  encode_world_create,
  LoadKind,
  read_fetch_create,
  read_fetch_delete,
  read_load_create,
  read_sensor_create,
  read_sensor_delete,
  read_world_snapshot,
  Sensor,
} from "@/kernel/abi/cmd.spec.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { storage } from "@/kernel/abi/host.spec.ts";
import { stored } from "@/kernel/host.ts";
import { hold_body } from "@/kernel/module.ts";
import {
  geolocation_start,
  geolocation_stop,
  geolocation_supported,
  orientation_listen,
  orientation_needs_permission,
  orientation_remove,
  orientation_silence_is_loss,
  orientation_supported,
} from "@platform/sensor.ts";
import { abort_fetch, run_fetch } from "@/kernel/io/fetch.ts";
import { load_image, load_model } from "@/kernel/io/load.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { CORE, load_core } from "@/kernel/mem.ts";
import { bytes_at } from "@/kernel/module.ts";
import { gpu_boot } from "@platform/gpu.ts";
import { start_sched } from "@/kernel/sched.ts";
import { kernel_snapshot, set_snapshot, subscribe_snapshot } from "@/kernel/snapshot.ts";
import { keepSettings, storedSettings } from "@/feature/setting/store.ts";

function on_signal(_tag: u32, cmd: Command, f: BranchCursor | null): void {
  switch (cmd) {
    case Command.WORLD_SNAPSHOT: {
      // The packet is far past a ring frame, so the kernel names the block
      // it holds it in. It is the kernel's and stays valid until it writes
      // the next one, which is a turn away.
      const { at, len } = read_world_snapshot(f!);
      set_snapshot(bytes_at(CORE.buffer, at, len));
      return;
    }
    case Command.WORLD_UPDATE:
      return;
    case Command.SENSOR_CREATE:
      if (read_sensor_create(f!).sensor === Sensor.Geolocation) geolocation_start();
      else orientation_listen();
      return;
    case Command.SENSOR_DELETE:
      if (read_sensor_delete(f!).sensor === Sensor.Geolocation) geolocation_stop();
      else orientation_remove();
      return;
    case Command.FETCH_CREATE: {
      const { key, bake, name } = read_fetch_create(f!);
      void run_fetch(key, bake, name);
      return;
    }
    case Command.FETCH_DELETE:
      abort_fetch(read_fetch_delete(f!).key);
      return;
    case Command.LOAD_CREATE: {
      const { key, kind, load } = read_load_create(f!);
      void (kind === LoadKind.Gltf ? load_model(key, load) : load_image(key, load));
      return;
    }
  }
  console.warn(`[kernel] signal 0x${cmd.toString(16)} with no one to hear it`);
}

/** The objects storage kept, held as a body for the kernel to copy in. */
function objects_body(): { objects: u32; objects_bytes: u32 } {
  const raw = stored(storage.OBJECTS);
  if (raw === null) return { objects: 0, objects_bytes: 0 };
  const bytes = new TextEncoder().encode(raw);
  return { objects: hold_body(bytes), objects_bytes: bytes.byteLength };
}

/** Starts the kernel: the module, the turns it is given and who hears what
 * a turn leaves, then a device to draw with and the world it draws. Called
 * once by the host's boot, so that what boots is ordered rather than
 * whichever module happened to be imported first. */
export async function boot_kernel(): Promise<void> {
  await load_core();
  start_sched();
  CORE_BUILD.signal = on_signal;
  gpu_boot();
  CORE_BUILD.call(
    Command.WORLD_CREATE,
    encode_world_create({
      development: __IS_DEV__,
      handheld: HANDHELD,
      geolocation: geolocation_supported(),
      orientation: orientation_supported(),
      orientation_permission: orientation_needs_permission(),
      orientation_silence_is_loss: orientation_silence_is_loss(),
      settings: storedSettings(),
      ...objects_body(),
    }),
    (result) => result,
  );
  // What the kernel holds from now on is what storage keeps: the snapshot
  // after WORLD_CREATE is the first to say it.
  subscribe_snapshot(() => keepSettings(kernel_snapshot().settings));
}
