// @pxl/client/ui/shared/watch.ts

import { useSyncExternalStore } from "react";

import type { Snapshot } from "@/abi/overlay.ts";
import {
  type UiView,
  watch_snapshot,
  watch_view,
} from "@/service/core/overlay.ts";
import { gpu, watch_gpu, type Gpu } from "@/service/sys/gpu.ts";
import { online, watch_online } from "@/service/origin/online.ts";
import {
  source_state,
  source_versions,
  watch_source,
  type Source,
} from "@/service/sys/fs/source.ts";
import * as overlay from "@/service/core/overlay.ts";

/** `select` must return a primitive or a reference that already exists. A new
 * object on every call makes React render again forever. */
export function useCore<T>(select: (s: Snapshot) => T): T {
  return useSyncExternalStore(watch_snapshot, () => select(overlay.cur()));
}

/** The view's azimuth and scale, which change apart from the snapshot.
 * `select` keeps the same rule as `useCore`'s. */
export function useView<T>(select: (v: UiView) => T): T {
  return useSyncExternalStore(watch_view, () => select(overlay.cur_view()));
}

export function useGpu(): Gpu {
  return useSyncExternalStore(watch_gpu, gpu);
}

export function useOnline(): boolean {
  return useSyncExternalStore(watch_online, online);
}

export function useSourceVersions(): ReturnType<typeof source_versions> {
  return useSyncExternalStore(watch_source, source_versions);
}

export function useSource(): Source {
  return useSyncExternalStore(watch_source, source_state);
}
