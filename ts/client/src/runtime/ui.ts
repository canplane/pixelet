// @pxl/client/runtime/ui.ts

/**
 * What the boot does alike on a page (boot.ts) and a device (boot.native.ts):
 * it mounts the interfaces the UI serves the core before the core starts, and
 * moves where the boot stands as the core answers and the world arrives. The
 * UI only reads where the boot stands (service/sys/boot.ts).
 */

import type { Remote } from "@xpute/link";
import { type Snapshot } from "@/abi/overlay.ts";
import { GROUND_STATUS_SUCCESS } from "@/abi/overlay.enum.ts";
import { set_online } from "@/service/origin/online.ts";
import { log } from "@/service/platform/log.ts";
import { stop_code } from "@/service/sys/abort.ts";
import {
  boot_state,
  set_boot,
  BOOT_FAILED,
  BOOT_LOADING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
} from "@/service/sys/boot.ts";
import { serve_overlay } from "@/service/core/core.ts";
import {
  SENSOR_LISTENER,
  serve_watch,
  tell_platform,
} from "@/service/core/sensor.ts";
import { listen } from "@/service/platform/sensor-listener.ts";
import { tell_dev } from "@/service/core/setting.ts";
import { cur, watch_snapshot } from "@/service/core/snapshot.ts";
import * as online from "@/service/platform/online.ts";

/** Whether the world has arrived, or nothing of it will show: every pipeline
 * is made, and the view is drawn or lies past the service area. When locating
 * fails, the view is drawn at its default place. */
function loaded(s: Snapshot): boolean {
  return (
    s.renderer_progress === 1 &&
    (s.pivot.status === GROUND_STATUS_SUCCESS || s.pivot.outside)
  );
}

/** Mounts the interfaces the UI serves on `remote`, before the core boots. */
export function serve_ui(remote: Remote): void {
  serve_overlay(remote);
  serve_watch(remote);
}

/** Starts the client, after the core is asked to start: the boot is STARTING,
 * the network's state is told the origin, the device's readings are routed to
 * the core, and the core is told the device's sensors and whether this is a
 * development build. Once both have answered the boot is LOADING, or FAILED
 * where either is refused, and STARTED once the world has arrived (`loaded`).
 * The core's calls wait until it has booted. */
export function start_client(): void {
  watch_snapshot(() => {
    if (boot_state() === BOOT_LOADING && loaded(cur())) set_boot(BOOT_STARTED);
  });
  online.watch(set_online);
  set_boot(BOOT_STARTING);
  // The device's readings go to the core from here on, before the core hears
  // that it has sensors and starts one.
  listen(SENSOR_LISTENER);
  Promise.all([tell_platform(), tell_dev()]).then(
    () => {
      if (boot_state() !== BOOT_STARTING) return;
      set_boot(BOOT_LOADING);
      // The world may have arrived before the core answered.
      if (loaded(cur())) set_boot(BOOT_STARTED);
    },
    (err: unknown) => {
      console.error("[boot] the client did not start:", err);
      if (boot_state() === BOOT_STARTING) set_boot(BOOT_FAILED);
    },
  );
}

/** The core trapped, with `err` as its runtime tells it: its turns end, and
 * the boot with them. */
export function trapped(err: string): void {
  log(`[core] stopped${stop_code() ? ` (${stop_code()})` : ""}: ${err}`);
  set_boot(BOOT_TERMINATED);
}
