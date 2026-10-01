// @pxl/web/kernel/kernel.ts

/**
 * The completion ring's signals, and the boot that starts everything.
 *
 * A turn leaves on the completion ring what the kernel asks of the host — the
 * sensors' watches — and the shell's snapshot when it changed, which the host
 * hands on; this is where each is handed to whoever runs it. Nothing is
 * decided here: every branch is one line to the module that does the work.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import type { BranchCursor } from "@xpute/core/wire/xtp/mod.ts";
import { Command, encode_world_boot, read_sensor_start, read_sensor_stop, read_world_snapshot, Sensor } from "@/kernel/abi/cmd.spec.ts";
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
import { declare_source } from "@/kernel/io/source.ts";
import { read_asset } from "@/kernel/io/load.ts";
import { read_object } from "@/kernel/io/fetch.ts";
import { bind_root } from "@/kernel/host.ts";
import { BAKE_KINDS } from "@/config/published.spec.ts";
import { CORE, CORE_DOORBELL, load_core } from "@/kernel/mem.ts";
import { root, storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import { storageGet } from "@platform/storage.ts";
import { bytes_at } from "@/kernel/module.ts";
import { gpuBoot } from "@platform/gpu.ts";
import { HANDHELD } from "@platform/core.ts";
import { start_sched } from "@/kernel/sched.ts";
import { set_snapshot } from "@/kernel/snapshot.ts";

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
    case Command.SENSOR_START:
      if (read_sensor_start(f!).sensor === Sensor.Geolocation) geolocationStart();
      else orientationListen();
      return;
    case Command.SENSOR_STOP:
      if (read_sensor_stop(f!).sensor === Sensor.Geolocation) geolocationStop();
      else orientationRemove();
      return;
  }
  console.warn(`[kernel] signal 0x${cmd.toString(16)} with no one to hear it`);
}

/** Starts the kernel: the module, the turns it is given and who hears what
 * a turn leaves, then a device to draw with and the world it draws. Called
 * once by the app's boot (app/bootstrap.ts), so that what boots is ordered
 * rather than whichever module happened to be imported first. */
export async function boot_kernel(): Promise<void> {
  await load_core();
  // A bake's root is its kind's number, below the roots of host.json's own.
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
      // The kernel's own file, handed over unread: it reads it, and writes it
      // from then on.
      settings: storageGet(STORAGE_KEYS[storage.SETTINGS]) ?? "",
    }),
    (result) => result,
  );
  // Not awaited: a dev tile server may answer late, and the kernel observes
  // nothing until it has, which the boot does not wait on.
  void declare_source();
}
