// @pxl/client/ui/shared/watch.ts

import { useSyncExternalStore } from "react";

import type { Snapshot } from "@/abi/overlay.ts";
import {
  type UiView,
  watchSnapshot,
  watchView,
} from "@/service/core/overlay.ts";
import { gpu, watchGpu, type Gpu } from "@/service/sys/gpu.ts";
import { online, watchOnline } from "@/service/origin/online.ts";
import {
  sourceState,
  sourceVersions,
  watchSource,
  type SourceState,
} from "@/service/sys/fs/source.ts";
import * as overlay from "@/service/core/overlay.ts";
import {
  type DragMode,
  dragMode,
  watchDragMode,
} from "@/service/core/input.ts";
import { watchSensor } from "@/service/core/sensor.ts";
import {
  type DeviceLocationStore,
  type DeviceOrientationStore,
  deviceLocationStore,
  deviceOrientationStore,
} from "@/service/core/sensor/reading.ts";

/** `select` must return a primitive or a reference that already exists. A new
 * object on every call makes React render again forever. */
export function useCore<T>(select: (s: Snapshot) => T): T {
  return useSyncExternalStore(watchSnapshot, () =>
    select(overlay.currentSnapshot()),
  );
}

/** The view's azimuth and scale, which change apart from the snapshot.
 * `select` keeps the same rule as `useCore`'s. */
export function useView<T>(select: (v: UiView) => T): T {
  return useSyncExternalStore(watchView, () => select(overlay.currentView()));
}

/** The device's sensors as their driver holds them. `select` keeps the same
 * rule as `useCore`'s. */
export function useSensor<T>(
  select: (
    location: DeviceLocationStore,
    orientation: DeviceOrientationStore,
  ) => T,
): T {
  return useSyncExternalStore(watchSensor, () =>
    select(deviceLocationStore(), deviceOrientationStore()),
  );
}

/** What the pointers hold the world for. */
export function useDragMode(): DragMode {
  return useSyncExternalStore(watchDragMode, dragMode);
}

export function useGpu(): Gpu {
  return useSyncExternalStore(watchGpu, gpu);
}

export function useOnline(): boolean {
  return useSyncExternalStore(watchOnline, online);
}

export function useSourceVersions(): ReturnType<typeof sourceVersions> {
  return useSyncExternalStore(watchSource, sourceVersions);
}

export function useSource(): SourceState {
  return useSyncExternalStore(watchSource, sourceState);
}
