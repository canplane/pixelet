// @pxl/client/host/boot.ts

/**
 * Hands the completion ring's signals to whoever runs them, and boots
 * everything. Nothing is decided here.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { Command, encode_world_boot, encode_world_kept_emptied, read_sensor_start, read_sensor_stop, read_world_snapshot, Sensor } from "@/host/abi/cmd.spec.ts";
import {
  geolocationStart,
  geolocationStop,
  geolocationSupported,
  orientationListen,
  orientationNeedsPermission,
  orientationRemove,
  orientationSilenceIsLoss,
  orientationSupported,
} from "@/bracket.tsx";
import { declare_source } from "@/host/io/source.ts";
import { bind_roots } from "@/host/io/roots.ts";
import { kept_refused, migrate_kept, read_kept } from "@/host/io/storage.ts";
import { GUEST, GUEST_DOORBELL, load_guest } from "@/host/mem.ts";
import { kept } from "@/host/abi/host.spec.ts";
import { bytes_at } from "@/host/module.ts";
import { gpuBoot } from "@/bracket.tsx";
import { onlineWatch } from "@/bracket.tsx";
import { send_next, start_sched, strobe } from "@/host/sched.ts";
import { kernel_snapshot, set_snapshot } from "@/host/snapshot.ts";

function on_signal(_tag: u32, cmd: Command, f: BranchCursor | null): void {
  switch (cmd) {
    case Command.WORLD_SNAPSHOT: {
      // Too large for a ring frame; valid until the kernel writes the next, a turn away.
      const { at, len } = read_world_snapshot(f!);
      set_snapshot(bytes_at(GUEST.buffer, at, len));
      // Sent after the completion ring is read: the doorbell refuses a write while it reads.
      kept_refused(kernel_snapshot().kept_refused, (slot) => queueMicrotask(() => send_next(Command.WORLD_KEPT_EMPTIED, encode_world_kept_emptied(GUEST_DOORBELL, slot))));
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

/** Called once from shell/bootstrap.ts, so boot order is explicit, not import order. */
export async function boot_kernel(): Promise<void> {
  await load_guest();
  bind_roots();
  start_sched();
  GUEST_DOORBELL.signal = on_signal;
  gpuBoot();
  onlineWatch();
  const boot = (settings: string) =>
    encode_world_boot(
      GUEST_DOORBELL,
      __IS_DEV__,
      geolocationSupported(),
      orientationSupported(),
      orientationNeedsPermission(),
      orientationSilenceIsLoss(),
      settings,
    );
  migrate_kept();
  // Storage is the user's to edit: kept settings too large to send are no
  // settings, and the kernel starts from the defaults, written over them.
  let pkt = boot(read_kept(kept.SETTINGS) ?? "");
  if (!GUEST_DOORBELL.fits(pkt)) pkt = boot("");
  const booted = GUEST_DOORBELL.call(Command.WORLD_BOOT, pkt, (result) => result);
  if (booted < 0) throw new Error(`WORLD_BOOT refused: ${strerror(booted)}`);
  // Not awaited: a dev tile server may answer late.
  void declare_source();
}

/** `listener` hears the kernel stop for good: its turns end with it. */
export function on_kernel_stop(listener: () => void): void {
  GUEST_DOORBELL.stopped = () => {
    strobe.stop();
    listener();
  };
}
