// @pxl/web/kernel/kernel.ts

/**
 * Hands the completion ring's signals to whoever runs them, and boots
 * everything. Nothing is decided here.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import type { BranchCursor } from "@xpute/core/wire/xtp/mod.ts";
import { Command, encode_world_boot, LoadKind, read_load_start, read_sensor_start, read_sensor_stop, read_world_snapshot, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { Command as XputeCommand, read_fetch_cancel, read_fetch_start } from "@xpute/runtime/abi/cmd.spec.ts";
import { storage } from "@/kernel/abi/host.spec.ts";
import { stored } from "@/kernel/host.ts";
import { BODIES } from "@/kernel/module.ts";
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
import { declare_source } from "@/kernel/io/source.ts";
import { load_image, load_model } from "@/kernel/io/load.ts";
import { CORE, CORE_DOORBELL, load_core } from "@/kernel/mem.ts";
import { bytes_at } from "@/kernel/module.ts";
import { gpuBoot } from "@platform/gpu.ts";
import { HANDHELD } from "@platform/core.ts";
import { start_sched } from "@/kernel/sched.ts";
import { kernel_snapshot, set_snapshot, subscribe_snapshot } from "@/kernel/snapshot.ts";
import { keepSettings, storedSettings } from "@/feature/setting/store.ts";

function on_signal(_tag: u32, cmd: Command | XputeCommand, f: BranchCursor | null): void {
  switch (cmd) {
    case Command.WORLD_SNAPSHOT: {
      // Too large for a ring frame; valid until the kernel writes the next, a turn away.
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
    case XputeCommand.FETCH_START: {
      const { key, source: bake, name } = read_fetch_start(f!);
      void run_fetch(key, bake, name);
      return;
    }
    case XputeCommand.FETCH_CANCEL:
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

/** Called once from app/bootstrap.ts, so boot order is explicit, not import order. */
export async function boot_kernel(): Promise<void> {
  await load_core();
  BAKE_KINDS.forEach((_, bake) => bind_root(bake, (name, signal) => read_object(bake, name, signal)));
  bind_root(root.ASSET, (path, signal) => read_asset(Number(path), signal));
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
      settings: storageGet(STORAGE_KEYS[storage.SETTINGS]) ?? "",
    }),
    (result) => result,
  );
  // Not awaited: a dev tile server may answer late.
  void declare_source();
  // What the kernel holds from now on is what storage keeps: the snapshot
  // after WORLD_BOOT is the first to say it.
  subscribe_snapshot(() => keepSettings(kernel_snapshot().settings));
}
