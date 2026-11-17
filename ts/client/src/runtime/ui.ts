// @pxl/client/runtime/ui.ts

/**
 * What the boot does alike on the web (boot.web.ts) and natively
 * (boot.native.ts): it mounts the interfaces the UI implements for the core
 * before the core starts, and moves where the boot stands as the core answers
 * and the world arrives. The UI only reads where the boot stands
 * (service/sys/boot.ts).
 */

import { type End, sender } from "@xpute/link";
import { type Snapshot } from "@/abi/overlay.ts";
import { setOnline } from "@/service/origin/online.ts";
import {
  bootState,
  setBoot,
  BOOT_LOADING,
  BOOT_OPENING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
} from "@/service/sys/boot.ts";
import { gpu, GPU_PRESENT, watchGpu } from "@/service/sys/gpu.ts";
import {
  sourceState,
  SOURCE_DECLARED,
  watchSource,
} from "@/service/sys/fs/source.ts";
import { setUi } from "@/service/core/core.ts";
import {
  SENSOR_LISTENER,
  sensorsVisibility,
  setPlatform,
} from "@/service/core/sensor.ts";
import { listen } from "@/service/platform/sensor-listener.ts";
import * as visibility from "@/service/platform/visibility.ts";
import {
  currentSnapshot,
  mountOverlay,
  watchSnapshot,
} from "@/service/core/overlay.ts";
import * as online from "@/service/platform/online.ts";

/** Whether the world has arrived, or nothing of it will show: every pipeline
 * is made, and the view is drawn or lies past the service area. When locating
 * fails, the view is drawn at its default place. */
function loaded(s: Snapshot): boolean {
  return s.renderer_progress === 1 && (s.pivot.ground_shown || s.pivot.outside);
}

/** Makes `ui` the UI's end and mounts on it the interfaces the UI
 * implements, before the core boots. */
export function mountUi(ui: End): void {
  setUi(sender(ui));
  mountOverlay(ui);
}

/** Moves the started core's boot on as what it waits on comes: LOADING once a
 * GPU is present and the source is declared, and STARTED once the world has
 * arrived (`loaded`). */
function step(): void {
  if (
    bootState() === BOOT_STARTING &&
    gpu() === GPU_PRESENT &&
    sourceState() === SOURCE_DECLARED
  )
    setBoot(BOOT_LOADING);
  if (bootState() === BOOT_LOADING && loaded(currentSnapshot()))
    setBoot(BOOT_STARTED);
}

/** Starts the client, after the core is asked to start: the boot moves on as
 * the core, the GPU and the source answer (`step`), the network's state is
 * told the origin, the device's readings and the app's hiding are routed to
 * the sensors' driver. The core's calls wait until it has booted. */
export function startClient(): void {
  // The core's overlay comes once it runs, at the end of its first entry or
  // whole as a UI attaches (pxl-core link.rs `post_ui`, `tell_all`): the
  // boot is STARTING from the first.
  watchSnapshot(() => {
    if (bootState() === BOOT_OPENING) setBoot(BOOT_STARTING);
    step();
  });
  watchGpu(step);
  watchSource(step);
  online.watch(setOnline);
  // The device's readings go to the driver from here on, before a world's
  // surface starts a sensor.
  listen(SENSOR_LISTENER);
  setPlatform();
  visibility.watch(sensorsVisibility);
}

/** The core trapped, whether before its first entry ended or after: its
 * turns end, and the boot with them. What stopped it is its runtime's to
 * write. */
export function trapped(): void {
  setBoot(BOOT_TERMINATED);
}
