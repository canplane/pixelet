// @pxl/web/kernel/kernel.ts

/**
 * The completion ring's signals, and the boot that starts everything.
 *
 * A turn leaves on the completion ring what the kernel asks of the host — the
 * sensors' watches, the fetches to start and cancel, the loads — and the
 * shell's snapshot when it changed, which the host hands on; this is where
 * each is handed to whoever runs it. Nothing is decided here: every branch is one line to the module that
 * does the work.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import type { BranchCursor } from "@xpute/core/wire/xtp/mod.ts";
import { Command, encode_world_boot, LoadKind, read_fetch_cancel, read_fetch_start, read_load_start, read_sensor_start, read_sensor_stop, read_world_snapshot, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { storage } from "@/kernel/abi/host.spec.ts";
import { stored } from "@/kernel/host.ts";
import { hold_body } from "@/kernel/module.ts";
import {
  geolocationStart,
  geolocationStop,
  geolocationSupported,
  orientationListen,
  orientationNeedsPermission,
  orientationRemove,
  orientationSilenceIsLoss,
  orientationSupported,
} from "@platform/sensor.ts";
import { abort_fetch, run_fetch } from "@/kernel/io/fetch.ts";
import { load_image, load_model } from "@/kernel/io/load.ts";
import { CORE, CORE_DOORBELL, load_core } from "@/kernel/mem.ts";
import { bytes_at } from "@/kernel/module.ts";
import { gpuBoot } from "@platform/gpu.ts";
import { HANDHELD } from "@platform/core.ts";
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
    case Command.WORLD_FRAME:
      return;
    case Command.SENSOR_START:
      if (read_sensor_start(f!).sensor === Sensor.Geolocation) geolocationStart();
      else orientationListen();
      return;
    case Command.SENSOR_STOP:
      if (read_sensor_stop(f!).sensor === Sensor.Geolocation) geolocationStop();
      else orientationRemove();
      return;
    case Command.FETCH_START: {
      const { key, bake, name } = read_fetch_start(f!);
      void run_fetch(key, bake, name);
      return;
    }
    case Command.FETCH_CANCEL:
      abort_fetch(read_fetch_cancel(f!).key);
      return;
    case Command.LOAD_START: {
      const { key, kind, load } = read_load_start(f!);
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
 * once by the app's boot (app/bootstrap.ts), so that what boots is ordered
 * rather than whichever module happened to be imported first. */
export async function boot_kernel(): Promise<void> {
  await load_core();
  start_sched();
  CORE_DOORBELL.signal = on_signal;
  gpuBoot();
  CORE_DOORBELL.call(
    Command.WORLD_BOOT,
    encode_world_boot({
      development: __IS_DEV__,
      handheld: HANDHELD,
      geolocation: geolocationSupported(),
      orientation: orientationSupported(),
      orientation_permission: orientationNeedsPermission(),
      orientation_silence_is_loss: orientationSilenceIsLoss(),
      settings: storedSettings(),
      ...objects_body(),
    }),
    (result) => result,
  );
  // What the kernel holds from now on is what storage keeps: the snapshot
  // after WORLD_BOOT is the first to say it.
  subscribe_snapshot(() => keepSettings(kernel_snapshot().settings));
}
