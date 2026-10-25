// @pxl/client/host/gpu.ts

import type { GpuHeard } from "@/bracket.tsx";

/** What the host has of a GPU, which the guest, given none, cannot say: one, none yet while it asks again, none it will ask for again, or none again after one it drew with. */
export const enum Gpu {
  Present = 0,
  Waiting = 1,
  Missing = 2,
  Lost = 3,
}

/** A place's word for its GPU, as the host keeps it. */
export const GPU_STATE: Readonly<Record<Parameters<GpuHeard["state"]>[0], Gpu>> = { present: Gpu.Present, waiting: Gpu.Waiting, missing: Gpu.Missing, lost: Gpu.Lost };

let state = Gpu.Present;
const listeners = new Set<() => void>();

export function gpu(): Gpu {
  return state;
}

export function set_gpu(gpu: Gpu): void {
  state = gpu;
  for (const listener of listeners) listener();
}

export function watch_gpu(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
