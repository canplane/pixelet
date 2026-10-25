// @pxl/client/shell/store/system.ts

/** What the shell reads of the system below it: the kernel's snapshot, the host's GPU and network, the source's versions. */

import { useSyncExternalStore } from "react";

import type { Snapshot } from "@/abi/cmd.spec.ts";
import { kernel_snapshot, watch_snapshot } from "@/abi/snapshot.ts";
import { type Gpu, gpu, watch_gpu } from "@/host/gpu.ts";
import { online, watch_online } from "@/host/online.ts";
import { type Source, source_state, source_versions, watch_source } from "@/namespace/source.ts";

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
