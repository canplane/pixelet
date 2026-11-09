// @pxl/client/ui/shared/watch.ts

import { useSyncExternalStore } from "react";

import type { UiView, Snapshot } from "@/abi/cmd.spec.ts";
import { watch_snapshot, watch_view } from "@/service/core/snapshot.ts";
import type { Gpu, Source } from "@/abi/state.ts";
import { gpu, watch_gpu } from "@/service/sys/gpu.ts";
import { online, watch_online } from "@/service/origin/online.ts";
import {
  source_state,
  source_versions,
  watch_source,
} from "@/service/sys/fs/source.ts";
import * as snapshot from "@/service/core/snapshot.ts";

/** `select` must return a primitive or a reference that already exists. A new
 * object on every call makes React render again forever. */
export function useCore<T>(select: (s: Snapshot) => T): T {
  return useSyncExternalStore(watch_snapshot, () => select(snapshot.cur()));
}

/** The view's azimuth and scale, which change apart from the snapshot.
 * `select` keeps the same rule as `useCore`'s. */
export function useView<T>(select: (v: UiView) => T): T {
  return useSyncExternalStore(watch_view, () => select(snapshot.cur_view()));
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
