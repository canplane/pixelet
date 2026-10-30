// @pxl/client/shell/store/watch.ts

/** A hook for each watch the layers below hand the shell (`watch_snapshot`, `watch_gpu`, `watch_online`,
 * `watch_source`): the kernel's snapshot, the GPU's and the network's state, the source's versions. */

import { useSyncExternalStore } from "react";

import type { Snapshot } from "@/abi/cmd.spec.ts";
import { kernel_snapshot, watch_snapshot } from "@/port/snapshot.ts";
import type { Gpu, Source } from "@/abi/state.ts";
import {
  gpu,
  online,
  source_state,
  source_versions,
  watch_gpu,
  watch_online,
  watch_source,
} from "@/port/shell.ts";

/** `select` must return a primitive or an existing reference; a fresh object renders forever. */
export function useKernel<T>(select: (s: Snapshot) => T): T {
  return useSyncExternalStore(watch_snapshot, () => select(kernel_snapshot()));
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
