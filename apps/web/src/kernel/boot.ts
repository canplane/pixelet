// @pxl/web/kernel/boot.ts

/**
 * Hands the completion ring's signals to whoever runs them, and boots
 * everything. Nothing is decided here.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { Command, encode_world_boot, read_sensor_start, read_sensor_stop, read_world_snapshot, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { geolocationStart, geolocationStop, geolocationSupported } from "@platform/sensor.ts";
import { declare_source } from "@/kernel/io/source.ts";
import { read_asset, read_object } from "@/kernel/io/read.ts";
import { bind_root, read_storage } from "@/kernel/host.ts";
import { BASE_LAYERS } from "@/config/published.spec.ts";
import { GUEST, GUEST_DOORBELL, load_guest } from "@/kernel/mem.ts";
import { root, SETTINGS_PATH } from "@/kernel/abi/host.spec.ts";
import { bytes_at } from "@/kernel/module.ts";
import { gpuBoot } from "@platform/gpu.ts";
import { HANDHELD } from "@platform/device.ts";
import { start_sched } from "@/kernel/sched.ts";
import { set_snapshot } from "@/kernel/snapshot.ts";

function on_signal(_tag: u32, cmd: Command, f: BranchCursor | null): void {
  switch (cmd) {
    case Command.WORLD_SNAPSHOT: {
      // Too large for a ring frame; valid until the kernel writes the next, a turn away.
      const { at, len } = read_world_snapshot(f!);
      set_snapshot(bytes_at(GUEST.buffer, at, len));
      return;
    }
    case Command.SENSOR_START:
      if (read_sensor_start(f!).sensor === Sensor.Geolocation) geolocationStart();
      return;
    case Command.SENSOR_STOP:
      if (read_sensor_stop(f!).sensor === Sensor.Geolocation) geolocationStop();
      return;
  }
  console.warn(`[kernel] signal 0x${cmd.toString(16)} with no one to hear it`);
}

/** Called once from app/bootstrap.ts, so boot order is explicit, not import order. */
export async function boot_kernel(): Promise<void> {
  await load_guest();
  BASE_LAYERS.forEach((_, bake) => {
    bind_root(bake, (name, signal) => read_object(bake, name, signal));
  });
  bind_root(root.ASSET, (path, signal) => read_asset(Number(path), signal));
  start_sched();
  GUEST_DOORBELL.signal = on_signal;
  gpuBoot();
  const boot = (settings: string) =>
    encode_world_boot({
      development: __IS_DEV__,
      handheld: HANDHELD,
      geolocation: geolocationSupported(),
      settings,
    });
  // Storage is the user's to edit: kept settings too large to send are no
  // settings, and the kernel starts from the defaults.
  const kept = boot(read_storage(SETTINGS_PATH) ?? "");
  const booted = GUEST_DOORBELL.call(Command.WORLD_BOOT, GUEST_DOORBELL.fits(kept) ? kept : boot(""), (result) => result);
  if (booted < 0) throw new Error(`WORLD_BOOT refused: ${strerror(booted)}`);
  // Not awaited: a dev tile server may answer late.
  void declare_source();
}
