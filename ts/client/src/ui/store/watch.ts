// @pxl/client/ui/store/watch.ts

/** A hook for each watch the layers below hand the UI (`watch_snapshot`, `watch_view`, `watch_gpu`,
 * `watch_online`, `watch_source`): the core's snapshot and its view, the GPU's and the network's state,
 * the source's versions. */

import { useSyncExternalStore } from "react";

import type { UiView, Snapshot } from "@/abi/cmd.spec.ts";
import { watch_snapshot, watch_view } from "@/port/snapshot.ts";
import type { Gpu, Source } from "@/abi/state.ts";
import {
  gpu,
  online,
  source_state,
  source_versions,
  watch_gpu,
  watch_online,
  watch_source,
} from "@/port/ui.ts";
import * as snapshot from "@/port/snapshot.ts";

/** `select` must return a primitive or an existing reference; a fresh object renders forever. */
export function useCore<T>(select: (s: Snapshot) => T): T {
  return useSyncExternalStore(watch_snapshot, () => select(snapshot.current()));
}

/** The view's azimuth and scale, apart from the snapshot; `select` as `useCore`'s. */
export function useView<T>(select: (v: UiView) => T): T {
  return useSyncExternalStore(watch_view, () =>
    select(snapshot.current_view()),
  );
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
