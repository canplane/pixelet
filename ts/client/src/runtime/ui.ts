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
  BOOT_OPENING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
} from "@/service/sys/boot.ts";
import { gpu, GPU_PRESENT, watch_gpu } from "@/service/sys/gpu.ts";
import {
  source_state,
  SOURCE_DECLARED,
  watch_source,
} from "@/service/sys/fs/source.ts";
import { set_ui } from "@/service/core/core.ts";
import { SENSOR_LISTENER, tell_platform } from "@/service/core/sensor.ts";
import { serve_watch } from "@/service/core/watch.ts";
import { listen } from "@/service/platform/sensor-listener.ts";
import { tell_dev } from "@/service/core/setting.ts";
import { cur, serve_overlay, watch_snapshot } from "@/service/core/overlay.ts";
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

/** Makes `remote` the UI's end of the link and mounts on it the interfaces the
 * UI serves, before the core boots. */
export function serve_ui(remote: Remote): void {
  set_ui(remote);
  serve_overlay(remote);
  serve_watch(remote);
}

/** Moves the started core's boot on as what it waits on comes: LOADING once a
 * GPU is present and the source is declared, and STARTED once the world has
 * arrived (`loaded`). */
function step(): void {
  if (
    boot_state() === BOOT_STARTING &&
    gpu() === GPU_PRESENT &&
    source_state() === SOURCE_DECLARED
  )
    set_boot(BOOT_LOADING);
  if (boot_state() === BOOT_LOADING && loaded(cur())) set_boot(BOOT_STARTED);
}

/** Starts the client, after the core is asked to start: the boot moves on as
 * the core, the GPU and the source answer (`step`), the network's state is
 * told the origin, the device's readings are routed to the core, and the core
 * is told the device's sensors and whether this is a development build, the
 * boot FAILED where either is refused while it opens or starts. The core's
 * calls wait until it has booted. */
export function start_client(): void {
  // The core's overlay comes once it runs, at the end of its first entry or
  // whole as a UI attaches (pxl-core link.rs `post_ui`, `tell_all`): the
  // boot is STARTING from the first.
  watch_snapshot(() => {
    if (boot_state() === BOOT_OPENING) set_boot(BOOT_STARTING);
    step();
  });
  watch_gpu(step);
  watch_source(step);
  online.watch(set_online);
  // The device's readings go to the core from here on, before the core hears
  // that it has sensors and starts one.
  listen(SENSOR_LISTENER);
  Promise.all([tell_platform(), tell_dev()]).catch((err: unknown) => {
    console.error("[boot] the client did not start:", err);
    const s = boot_state();
    if (s === BOOT_OPENING || s === BOOT_STARTING) set_boot(BOOT_FAILED);
  });
}

/** The core trapped, with `err` as its runtime tells it: its turns end, and
 * the boot with them. */
export function trapped(err: string): void {
  log(`[core] stopped${stop_code() ? ` (${stop_code()})` : ""}: ${err}`);
  set_boot(BOOT_TERMINATED);
}
